//! Touch input (`horizon_client::touch`) applied to the game: the pointer and its buttons as mouse
//! events, the view turned as by a grabbed mouse, and buttons as the vanilla key mappings they
//! stand for.
use super::{Game, Loading, Screen};
use horizon_client::{
    Result,
    hud::HudBatch,
    keymapping::{SNEAK, SPRINT},
    touch::{self as fingers, Context, Here, Input, Layout, Mode, Phase, TouchEvent},
};
use serde_json::{Value, json};
use std::time::Instant;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::{DeviceId, ElementState, MouseButton, Touch, TouchPhase, WindowEvent},
    event_loop::ActiveEventLoop,
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

/// The mouse event one of the touch layer's pointer inputs stands for.
fn mouse_event(device_id: DeviceId, input: &Input) -> Option<WindowEvent> {
    let button = |button, state| WindowEvent::MouseInput {
        device_id,
        state,
        button: match button {
            fingers::MouseButton::Left => MouseButton::Left,
            fingers::MouseButton::Right => MouseButton::Right,
            fingers::MouseButton::Middle => MouseButton::Middle,
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
        let event = touch_event(&touch, Here::default());
        let inputs = self.touch.touch(&layout, event, Instant::now());
        self.apply_touch(event_loop, id, touch.device_id, inputs);
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
        };
        let inputs = self.touch.frame(context, Instant::now());
        self.apply_touch(event_loop, id, DeviceId::dummy(), inputs);
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
        for input in inputs {
            let lets_go = matches!(
                input,
                Input::Pointer(_)
                    | Input::Release(_)
                    | Input::Abort(_)
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
                    if let Some(event) = mouse_event(device_id, &input) {
                        self.window_event(event_loop, id, event);
                    }
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
            };
            if let Err(e) = result {
                log::warn!("Horizon: touch: {e}");
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
        let event = touch_event(&touch, Here::default());
        let inputs = self.touch.touch(&layout, event, Instant::now());
        for input in inputs {
            if let Some(event) = mouse_event(touch.device_id, &input) {
                self.menu_event(event_loop, event)?;
            }
        }
        Ok(())
    }
}
