//! Touch input (`horizon_client::touch`) applied to the game: the pointer and its buttons as mouse
//! events, the view turned as by a grabbed mouse, and buttons as the vanilla key mappings they
//! stand for.
use super::{Game, Loading, Screen};
use horizon_client::{
    Result,
    creative::Creative,
    debug_keys::DebugAction,
    hud::HudBatch,
    keymapping::{SNEAK, SPRINT},
    touch::{
        self as fingers, Context, Function, Here, Input, Layout, Mode, Phase, SlotKind, SlotSpot,
        Spot, TouchEvent,
    },
    widgets::{self, Kind},
};
use serde_json::{Value, json};
use std::time::Instant;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::{DeviceId, ElementState, MouseButton, Touch, TouchPhase, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::KeyCode,
    window::WindowId,
};

pub(super) const HELP: &[&str] = &[
    "touch start|move|end|cancel ID X Y    a finger at GUI pixels X Y, as a touch screen reports it",
];

fn phase(phase: TouchPhase) -> Phase {
    match phase {
        TouchPhase::Started => Phase::Start,
        TouchPhase::Moved => Phase::Move,
        TouchPhase::Ended => Phase::End,
        TouchPhase::Cancelled => Phase::Cancel,
    }
}

/// The mouse event one of the touch layer's pointer inputs stands for. Its middle button is the
/// pick block action, whatever that is bound to, which `Game::touch_pick` drives without one.
fn mouse_event(device_id: DeviceId, input: &Input) -> Option<WindowEvent> {
    let button = |button, state| {
        let button = match button {
            fingers::MouseButton::Left => MouseButton::Left,
            fingers::MouseButton::Right => MouseButton::Right,
            fingers::MouseButton::Middle => return None,
        };
        Some(WindowEvent::MouseInput {
            device_id,
            state,
            button,
        })
    };
    match *input {
        Input::Pointer(at) => Some(WindowEvent::CursorMoved {
            device_id,
            position: PhysicalPosition::new(at[0], at[1]),
        }),
        Input::Press { button: b, .. } => button(b, ElementState::Pressed),
        Input::Release(b) | Input::Abort(b) => button(b, ElementState::Released),
        _ => None,
    }
}

/// The creative screen's trash slot, which has no slot id of its own.
const TRASH: u32 = u32::MAX;

/// What last frame's widgets show at `p` (GUI pixels, `scale` physical pixels each): a list
/// that scrolls and its scrollbar, sliders and text fields.
fn widget_here(p: (f32, f32), scale: f32) -> Here {
    let inside = |[x, y, w, h]: [f32; 4]| (x..x + w).contains(&p.0) && (y..y + h).contains(&p.1);
    let list = widgets::list_at(p.0, p.1);
    let target = widgets::last_targets()
        .into_iter()
        .rev()
        .find(|t| inside(t.rect));
    let spot = match target {
        _ if list.and_then(|l| l.bar).is_some_and(inside) => Spot::Grab,
        Some(t) if matches!(t.kind, Kind::Slider { .. }) => Spot::Slider,
        Some(t) if t.kind == Kind::Field => Spot::Field,
        _ => Spot::Plain,
    };
    Here {
        spot,
        list: list.map(|l| l.line * scale),
        ..Here::default()
    }
}

fn touch_event(touch: &Touch, here: Here) -> TouchEvent {
    TouchEvent {
        id: touch.id,
        phase: phase(touch.phase),
        at: [touch.location.x, touch.location.y],
        here,
    }
}

impl Game<'_> {
    fn touch_layout(&self) -> Option<Layout> {
        let r = self.renderer.as_ref()?;
        let scale = self.window.as_ref()?.scale_factor();
        Some(Layout::new(r.config.width, r.config.height, scale))
    }

    /// The held item is used over time, as food, potions, bows and shields are
    /// (`Item.getUseDuration`): a held finger uses it rather than mining.
    fn held_item_used_over_time(&self) -> bool {
        let Some(stack) = self.held() else {
            return false;
        };
        let components = self
            .pack
            .item_renderer
            .models
            .components(&horizon_client::item_render::stack_view(stack));
        super::item_draw::use_duration(&components) > 0
            || components
                .get("minecraft:item_model")
                .and_then(Value::as_str)
                == Some("minecraft:shield")
    }

    pub(super) fn touch_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, touch: Touch) {
        self.last_input = Instant::now();
        let Some(layout) = self.touch_layout() else {
            return;
        };
        let here = if self.pad_in_menu() {
            self.touch_here([touch.location.x, touch.location.y])
        } else {
            Here::default()
        };
        let context = self.touch_context();
        let inputs = self
            .touch
            .touch(&layout, context, touch_event(&touch, here), Instant::now());
        self.apply_touch(event_loop, id, touch.device_id, inputs);
    }

    /// What lies under a finger at `at` (physical pixels) on the open menu. It hit-tests the
    /// point, not `self.cursor`, which hasn't moved there yet.
    fn touch_here(&mut self, at: [f64; 2]) -> Here {
        let Some(batch) = self.gui_scale() else {
            return Here::default();
        };
        let p = (at[0] as f32 / batch.scale, at[1] as f32 / batch.scale);
        let mut here = widget_here(p, batch.scale);
        if self.inventory_open() {
            if let Some(spot) = self.inventory_spot(&batch, p) {
                here.spot = spot;
            }
            here.carrying = self.carried.is_some();
        }
        here
    }

    /// The first that claims the point on an inventory: creative's tabs (beyond the panel, but
    /// theirs), trash and scrollbar, then beyond the panel (as `Target::of` has it), a slot.
    fn inventory_spot(&mut self, batch: &HudBatch, p: (f32, f32)) -> Option<Spot> {
        if self.screen == Screen::Creative {
            let (ox, oy) = Self::creative_origin(batch);
            let on_tab = self.creative_screen.tabs.iter().any(|t| {
                let (tx, ty) = Creative::tab_position(t);
                (ox + tx..ox + tx + 26.0).contains(&p.0) && (oy + ty..oy + ty + 32.0).contains(&p.1)
            });
            if on_tab {
                return Some(Spot::Plain);
            }
            if self.creative_inventory_open() && Self::creative_trash_hovered(batch, p) {
                return Some(Spot::Slot(SlotSpot {
                    id: TRASH,
                    filled: false,
                    kind: SlotKind::Trash,
                }));
            }
            if self.creative_screen.inside_scrollbar(p.0 - ox, p.1 - oy) {
                return Some(Spot::Grab);
            }
        }
        if self.recipe_alternatives_at(p) {
            return Some(Spot::Secondary);
        }
        // A book covering the menu takes every press, beyond the panel too.
        if self.recipe_book_covers() {
            return None;
        }
        if self.clicked_outside_at(batch, p) {
            return Some(Spot::Outside);
        }
        let id = self.slot_at(batch, p.0, p.1)?;
        let (kind, filled) = match id {
            id if id >= Creative::DISPLAY => (SlotKind::Display, self.creative_display_filled(id)),
            id if self.output_slot(id) => (SlotKind::Result, self.slot_view(id).is_some()),
            _ => (SlotKind::Normal, self.slot_view(id).is_some()),
        };
        Some(Spot::Slot(SlotSpot {
            id: id as u32,
            filled,
            kind,
        }))
    }

    /// Where the carried stack is drawn (`draw_carried`): under the pointer, or, while a touch
    /// screen's pointer is away, where the last finger lifted, so a stack picked up with a tap
    /// stays in sight.
    pub(super) fn carried_anchor(&self) -> (f32, f32) {
        match (self.touch.rest(), self.gui_scale()) {
            (Some([x, y]), Some(batch)) if self.cursor.0 < 0.0 && self.cursor.1 < 0.0 => {
                (x as f32 / batch.scale, y as f32 / batch.scale)
            }
            _ => self.cursor,
        }
    }

    /// What the game shows the touch controls.
    fn touch_context(&self) -> Context {
        let mode = if self.title.is_some() {
            Mode::Title
        } else if self.pad_in_menu() || self.server_packs.prompt.is_some() {
            Mode::Menu
        } else {
            Mode::Playing
        };
        Context {
            mode,
            scene: super::scene_id(self.scene()),
            movement: self.settings.controller.touch_movement,
            flying: self.player.as_ref().is_some_and(|p| p.flying),
            held_item_used_over_time: self.held_item_used_over_time(),
            slots: self.inventory_open(),
            hud_visible: !self.ui.hide_gui,
        }
    }

    /// Once a frame, after the pads: what the screen shows, and held fingers.
    pub(super) fn touch_frame(&mut self, event_loop: &ActiveEventLoop) {
        let Some(id) = self.window.as_ref().map(|w| w.id()) else {
            return;
        };
        // A thumb resting on the joystick is play, as a tilted stick is (`poll_pads`).
        if self.touch.active() {
            self.last_input = Instant::now();
        }
        let context = self.touch_context();
        let inputs = self.touch.frame(context, Instant::now());
        self.apply_touch(event_loop, id, DeviceId::dummy(), inputs);
        if let (Some(layout), Some(batch)) = (self.touch_layout(), self.gui_scale()) {
            horizon_client::ui::set_tooltip_lift(self.touch.tooltip_lift(&layout) / batch.scale);
        }
    }

    fn apply_touch(
        &mut self,
        event_loop: &ActiveEventLoop,
        id: WindowId,
        device_id: DeviceId,
        inputs: Vec<Input>,
    ) {
        // While a world opens or saves, presses, clicks and looking are dropped; releases still
        // go, so the touch state and the game agree, and the pointer meets window_event's checks.
        let busy = self.pending_save.is_some()
            || self.world_loader.is_some()
            || self.pending_entry.is_some();
        // A click that changes the scene ends the batch's clicks: the rest were meant for the
        // screen it left, and a button still to come up comes up clicking nothing.
        let before = self.scene();
        let mut changed = false;
        for input in inputs {
            let input = match input {
                Input::Release(button) if changed => Input::Abort(button),
                Input::Press { .. } | Input::PutDown(_) | Input::Wheel(_) | Input::Function(_)
                    if changed =>
                {
                    continue;
                }
                input => input,
            };
            let clicks = matches!(
                input,
                Input::Press { .. }
                    | Input::Release(_)
                    | Input::Abort(_)
                    | Input::PutDown(_)
                    | Input::Wheel(_)
                    | Input::Function(_)
            );
            let lets_go = matches!(
                input,
                Input::Pointer(_)
                    | Input::Release(_)
                    | Input::Abort(_)
                    | Input::Shift(false)
                    | Input::Mapping(_, false)
                    | Input::Sprint(false)
            );
            if busy && !lets_go {
                continue;
            }
            match input {
                // A press of the touch layer's own never makes a double click (PICKUP_ALL).
                Input::Press { fresh: true, .. } => self.inv.last_click = None,
                // Whatever a mouse button's release would do to an inventory is undone first.
                Input::Abort(_) => self.inventory_abort(),
                _ => {}
            }
            let result = match input {
                Input::Press {
                    button: fingers::MouseButton::Middle,
                    ..
                } => {
                    self.inventory_pick(true);
                    Ok(())
                }
                Input::Release(fingers::MouseButton::Middle)
                | Input::Abort(fingers::MouseButton::Middle) => {
                    self.inventory_pick(false);
                    Ok(())
                }
                // The game mode switcher answers no mouse click; a finger picks a mode there.
                Input::Release(fingers::MouseButton::Left)
                    if self.screen == Screen::GameModeSwitcher =>
                {
                    self.touch_choose_mode();
                    Ok(())
                }
                Input::Pointer(_) | Input::Press { .. } | Input::Release(_) | Input::Abort(_) => {
                    if let Some(event) = mouse_event(device_id, &input) {
                        self.window_event(event_loop, id, event);
                    }
                    Ok(())
                }
                Input::PutDown(at) => {
                    self.inventory_put_down(event_loop, id, device_id, at);
                    Ok(())
                }
                Input::Shift(down) => {
                    self.touch_shift(down);
                    Ok(())
                }
                // A finger's lines are its own: the mouse wheel's sensitivity and Discrete
                // Scrolling don't change how far a drag scrolls.
                Input::Wheel(lines) => {
                    self.screen_wheel(0.0, lines);
                    Ok(())
                }
                Input::Keyboard => {
                    self.request_keyboard();
                    Ok(())
                }
                Input::Look([dx, dy]) => {
                    self.mouse.add_motion(dx, dy);
                    Ok(())
                }
                Input::Mapping(mapping, down) => self.mapping_action(mapping, down),
                Input::Sprint(on) => {
                    self.touch_sprint(on);
                    Ok(())
                }
                Input::Sneak => self.touch_sneak(),
                Input::Tap => self.touch_tap(),
                Input::Hotbar(slot) => {
                    if self.screen == Screen::Playing {
                        self.selected = usize::from(slot);
                    }
                    Ok(())
                }
                Input::Back => self.pad_back(),
                Input::Function(function) => self.touch_function(function),
            };
            if let Err(e) = result {
                log::warn!("Horizon: touch: {e}");
            }
            changed |= clicks && self.scene() != before;
        }
    }

    /// Shift down or up around the touch controls' clicks, in `keys` and `physical_keys` as the
    /// pad's quick move has it; a Shift a keyboard holds stays held.
    fn touch_shift(&mut self, down: bool) {
        let shift = KeyCode::ShiftLeft;
        if down {
            self.touch_shift.get_or_insert((
                self.keys.contains(&shift),
                self.physical_keys.contains(&shift),
            ));
            self.keys.insert(shift);
            self.physical_keys.insert(shift);
        } else if let Some((keys, physical)) = self.touch_shift.take() {
            if !keys {
                self.keys.remove(&shift);
            }
            if !physical {
                self.physical_keys.remove(&shift);
            }
        }
    }

    /// Sprinting as the joystick wants it, set outright: under Sprint: Toggle a key's edges
    /// would flip it, and behind a screen they don't reach it.
    fn touch_sprint(&mut self, on: bool) {
        self.toggles.set(SPRINT, on);
        self.sync_toggle(SPRINT);
    }

    /// The Sneak button flips sneaking: under Sneak: Toggle a press does, as for the pad's stick
    /// click (`pad_action`); otherwise the key stays down until the next tap.
    fn touch_sneak(&mut self) -> Result<()> {
        if self.settings.toggle_crouch {
            self.mapping_action("key.sneak", true)?;
            self.mapping_action("key.sneak", false)
        } else {
            let sneaking = self.toggles.is_down(SNEAK);
            self.mapping_action("key.sneak", !sneaking)
        }
    }

    /// A tap on the view clicks as Bedrock's does: an attack on a living entity under the
    /// crosshair (`crosshairPickEntity`), a use on anything else.
    fn touch_tap(&mut self) -> Result<()> {
        let mapping = if self.crosshair_living().is_some() {
            "key.attack"
        } else {
            "key.use"
        };
        self.mapping_action(mapping, true)?;
        self.mapping_action(mapping, false)
    }

    /// A key cap or a tile of the Fn panel: its keys pressed and released whole, F1, F2 and F5
    /// as their key mappings, F3 and its combinations as the debug keys.
    fn touch_function(&mut self, function: Function) -> Result<()> {
        let combination = match function {
            Function::Perspective => return self.tap_mapping("key.togglePerspective"),
            Function::HideHud => return self.tap_mapping("key.toggleGui"),
            Function::Screenshot => return self.tap_mapping("key.screenshot"),
            Function::DebugScreen => None,
            Function::Hitboxes => Some(DebugAction::ShowHitboxes),
            Function::ChunkBorders => Some(DebugAction::ShowChunkBorders),
            Function::AdvancedTooltips => Some(DebugAction::AdvancedTooltips),
            Function::GameModes => Some(DebugAction::SwitchGameMode),
        };
        self.touch_debug(combination);
        Ok(())
    }

    /// A key mapping pressed and released, the release sent even if the press failed.
    fn tap_mapping(&mut self, mapping: &str) -> Result<()> {
        let pressed = self.mapping_action(mapping, true);
        self.mapping_action(mapping, false)?;
        pressed
    }

    /// The key caps, or the Fn panel's tiles while it is open, for `ui`: in GUI pixels.
    pub(super) fn touch_functions_value(&self) -> Value {
        let (Some(layout), Some(batch)) = (self.touch_layout(), self.gui_scale()) else {
            return json!([]);
        };
        let shown: Vec<Value> = self
            .touch
            .functions(&layout)
            .iter()
            .map(|s| {
                json!({
                    "keys": s.keys,
                    "label": s.does,
                    "bounds": s.rect.map(|v| v / batch.scale),
                })
            })
            .collect();
        json!(shown)
    }

    pub(super) fn draw_touch(&self, batch: &mut HudBatch) {
        // An F2 screenshot is of the game, not of the controls over it.
        if self.ui.screenshot_message.is_some() {
            return;
        }
        if let Some(layout) = self.touch_layout() {
            let sneaking = self.toggles.is_down(SNEAK);
            self.touch
                .draw(batch, &self.gui.atlas, &layout, sneaking, Instant::now());
        }
    }

    /// `horizon-ctl touch ...`: a finger as the screen reports one, in GUI pixels.
    pub(super) fn touch_command(
        &mut self,
        event_loop: &ActiveEventLoop,
        words: &[&str],
    ) -> Result<Value> {
        let [_, phase, finger, x, y] = words else {
            return Err(format!("usage: {}", HELP[0]).into());
        };
        let phase = match *phase {
            "start" => TouchPhase::Started,
            "move" => TouchPhase::Moved,
            "end" => TouchPhase::Ended,
            "cancel" => TouchPhase::Cancelled,
            other => {
                return Err(
                    format!("touch phase must be start, move, end or cancel, not {other}").into(),
                );
            }
        };
        let id = self.window.as_ref().ok_or("no window yet")?.id();
        let scale = self.gui_scale().ok_or("no renderer yet")?.scale;
        let (x, y): (f64, f64) = (x.parse()?, y.parse()?);
        let touch = Touch {
            device_id: DeviceId::dummy(),
            phase,
            location: PhysicalPosition::new(x * f64::from(scale), y * f64::from(scale)),
            force: None,
            id: finger.parse()?,
        };
        self.touch_event(event_loop, id, touch);
        Ok(json!({ "ok": true }))
    }
}

impl Loading {
    /// The title screen while the game loads: a finger is the pointer.
    pub(super) fn touch_event(&mut self, event_loop: &ActiveEventLoop, touch: Touch) -> Result<()> {
        let (Some(r), Some(window)) = (&self.renderer, &self.window) else {
            return Ok(());
        };
        let layout = Layout::new(r.config.width, r.config.height, window.scale_factor());
        let scale = HudBatch::new([r.config.width, r.config.height]).scale;
        let p = (
            touch.location.x as f32 / scale,
            touch.location.y as f32 / scale,
        );
        let event = touch_event(&touch, widget_here(p, scale));
        let context = Context {
            mode: Mode::Title,
            scene: self.scene(),
            ..Context::default()
        };
        let inputs = self.touch.touch(&layout, context, event, Instant::now());
        for input in inputs {
            match input {
                Input::Pointer(_) | Input::Press { .. } | Input::Release(_) | Input::Abort(_) => {
                    if let Some(event) = mouse_event(touch.device_id, &input) {
                        self.menu_event(event_loop, event)?;
                    }
                }
                Input::Wheel(lines) => self.menu_wheel(lines),
                Input::Keyboard => self.request_keyboard(),
                // Nothing of a world or an inventory is on the title screen.
                Input::Shift(_)
                | Input::PutDown(_)
                | Input::Look(_)
                | Input::Mapping(..)
                | Input::Sprint(_)
                | Input::Sneak
                | Input::Tap
                | Input::Hotbar(_)
                | Input::Back
                | Input::Function(_) => {}
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pick_action_reaches_no_mouse_binding() {
        let device = DeviceId::dummy();
        let middle = fingers::MouseButton::Middle;
        let press = Input::Press {
            button: middle,
            fresh: true,
        };
        assert!(mouse_event(device, &press).is_none());
        assert!(mouse_event(device, &Input::Release(middle)).is_none());
        assert!(matches!(
            mouse_event(device, &Input::Abort(fingers::MouseButton::Right)),
            Some(WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Right,
                ..
            })
        ));
    }

    #[test]
    fn widgets_drawn_last_frame_say_what_a_finger_is_on() {
        widgets::end_frame(1);
        widgets::target_as([10.0, 10.0, 100.0, 20.0], Kind::Field);
        let slider = Kind::Slider {
            value: 0.5,
            track: [10.0, 100.0],
        };
        widgets::target_as([10.0, 60.0, 100.0, 20.0], slider);
        widgets::list(widgets::List {
            rect: [0.0, 50.0, 200.0, 100.0],
            bar: Some([150.0, 50.0, 6.0, 100.0]),
            line: 18.0,
        });
        widgets::end_frame(2);
        let at = |x, y| widget_here((x, y), 5.0);
        assert_eq!(at(20.0, 20.0).spot, Spot::Field);
        assert_eq!(at(20.0, 20.0).list, None);
        assert_eq!(at(20.0, 70.0).spot, Spot::Slider);
        assert_eq!(at(20.0, 70.0).list, Some(90.0), "a line in physical pixels");
        assert_eq!(at(152.0, 70.0).spot, Spot::Grab);
        assert_eq!(at(120.0, 120.0).spot, Spot::Plain);
        assert_eq!(at(120.0, 120.0).list, Some(90.0));
        assert_eq!(at(120.0, 170.0), Here::default());
    }
}
