//! Touch input (`horizon_client::touch`) applied to the game: the pointer and its buttons as mouse
//! events, the view turned as by a grabbed mouse, and buttons as the vanilla key mappings they
//! stand for.
use super::{Game, Loading, Screen};
use horizon_client::{
    Result,
    creative::Creative,
    hud::HudBatch,
    keymapping::{SNEAK, SPRINT},
    touch::{
        self as fingers, Context, Here, Input, Layout, Mode, Phase, SlotKind, SlotSpot, Spot,
        TouchEvent,
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

/// The mouse event one of the touch layer's pointer inputs stands for. `pick` is the mouse button
/// the pick block mapping is bound to, which the touch layer's middle button stands for.
fn mouse_event(device_id: DeviceId, input: &Input, pick: MouseButton) -> Option<WindowEvent> {
    let button = |button, state| WindowEvent::MouseInput {
        device_id,
        state,
        button: match button {
            fingers::MouseButton::Left => MouseButton::Left,
            fingers::MouseButton::Right => MouseButton::Right,
            fingers::MouseButton::Middle => pick,
        },
    };
    Some(match *input {
        Input::Pointer(at) => WindowEvent::CursorMoved {
            device_id,
            position: PhysicalPosition::new(at[0], at[1]),
        },
        Input::Press { button: b, .. } => button(b, ElementState::Pressed),
        Input::Release(b) | Input::Abort(b) => button(b, ElementState::Released),
        _ => return None,
    })
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
        let inputs = self
            .touch
            .touch(&layout, touch_event(&touch, here), Instant::now());
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
        if self.title.is_none() && matches!(self.screen, Screen::Inventory | Screen::Creative) {
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
        if self.clicked_outside_at(batch, p) {
            return Some(Spot::Outside);
        }
        let id = self.slot_at(batch, p.0, p.1)?;
        let (kind, filled) = match id {
            200 => (SlotKind::Result, self.slot_view(id).is_some()),
            id if id >= Creative::DISPLAY => (SlotKind::Display, self.creative_display_filled(id)),
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

    /// Once a frame, after the pads: what the screen shows, and held fingers.
    pub(super) fn touch_frame(&mut self, event_loop: &ActiveEventLoop) {
        let Some(id) = self.window.as_ref().map(|w| w.id()) else {
            return;
        };
        let mode = if self.title.is_some() {
            Mode::Title
        } else if self.pad_in_menu() || self.server_packs.prompt.is_some() {
            Mode::Menu
        } else {
            Mode::Playing
        };
        // A thumb resting on the joystick is play, as a tilted stick is (`poll_pads`).
        if self.touch.active() {
            self.last_input = Instant::now();
        }
        let context = Context {
            mode,
            movement: self.settings.controller.touch_movement,
            flying: self.player.as_ref().is_some_and(|p| p.flying),
            held_item_used_over_time: self.held_item_used_over_time(),
            slots: self.title.is_none()
                && matches!(self.screen, Screen::Inventory | Screen::Creative),
        };
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
        let pick = horizon_client::keybinds::mouse_button(self.settings.keys.get("key.pickItem"))
            .filter(|b| !matches!(b, MouseButton::Left | MouseButton::Right))
            .unwrap_or(MouseButton::Middle);
        // While a world opens or saves, presses, clicks and looking are dropped; releases still
        // go, so the touch state and the game agree, and the pointer meets window_event's checks.
        let busy = self.pending_save.is_some()
            || self.world_loader.is_some()
            || self.pending_entry.is_some();
        // A click that changes the screen ends the batch's clicks: the rest were meant for the
        // screen it left.
        let scene = |game: &Self| (game.screen, game.title.is_some());
        let before = scene(self);
        let mut changed = false;
        for input in inputs {
            let clicks = matches!(
                input,
                Input::Press { .. }
                    | Input::Release(_)
                    | Input::Abort(_)
                    | Input::PutBack(_)
                    | Input::Wheel(_)
            );
            if changed && clicks {
                continue;
            }
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
                Input::Press { fresh: true, .. } => self.last_inv_click = None,
                // Whatever a mouse button's release would do to the inventory is undone first.
                Input::Abort(_) if matches!(self.screen, Screen::Inventory | Screen::Creative) => {
                    self.inv_drag = None;
                    self.inv_clicks.skip_release();
                }
                _ => {}
            }
            let result = match input {
                Input::Pointer(_) | Input::Press { .. } | Input::Release(_) | Input::Abort(_) => {
                    if let Some(event) = mouse_event(device_id, &input, pick) {
                        self.window_event(event_loop, id, event);
                    }
                    Ok(())
                }
                Input::PutBack(at) => {
                    if self.carried.is_some() {
                        self.last_inv_click = None;
                        let left = fingers::MouseButton::Left;
                        let click = [
                            Input::Pointer(at),
                            Input::Press {
                                button: left,
                                fresh: true,
                            },
                            Input::Release(left),
                        ];
                        for input in &click {
                            if let Some(event) = mouse_event(device_id, input, pick) {
                                self.window_event(event_loop, id, event);
                            }
                        }
                    }
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
                Input::Keyboard => Ok(()),
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
            };
            if let Err(e) = result {
                log::warn!("Horizon: touch: {e}");
            }
            changed |= clicks && scene(self) != before;
        }
        if changed {
            let inputs = self.touch.spend();
            self.apply_touch(event_loop, id, device_id, inputs);
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

    pub(super) fn draw_touch(&self, batch: &mut HudBatch) {
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
        let inputs = self.touch.touch(&layout, event, Instant::now());
        for input in inputs {
            if let Input::Wheel(lines) = input {
                self.menu_wheel(lines);
            } else if let Some(event) = mouse_event(touch.device_id, &input, MouseButton::Middle) {
                self.menu_event(event_loop, event)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
