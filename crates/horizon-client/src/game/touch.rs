//! Touch input (`horizon_client::touch`) applied to the game: the pointer as mouse events, the
//! view turned as by a grabbed mouse, and buttons as the vanilla key mappings they stand for.
use super::{Game, Loading, Screen};
use horizon_client::{
    Result,
    hud::HudBatch,
    touch::{Input, Layout, Mode, Phase},
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

pub(super) const HELP: &[&str] =
    &["touch start|move|end ID X Y    a finger at GUI pixels X Y, as a touch screen reports it"];

fn phase(phase: TouchPhase) -> Phase {
    match phase {
        TouchPhase::Started => Phase::Start,
        TouchPhase::Moved => Phase::Move,
        TouchPhase::Ended | TouchPhase::Cancelled => Phase::End,
    }
}

/// The mouse events a touch pointer stands for: a move there, then the left button's press or
/// release.
fn pointer_events(
    device_id: DeviceId,
    at: [f64; 2],
    pressed: Option<bool>,
) -> impl Iterator<Item = WindowEvent> {
    let moved = WindowEvent::CursorMoved {
        device_id,
        position: PhysicalPosition::new(at[0], at[1]),
    };
    let button = pressed.map(|pressed| WindowEvent::MouseInput {
        device_id,
        state: if pressed {
            ElementState::Pressed
        } else {
            ElementState::Released
        },
        button: MouseButton::Left,
    });
    std::iter::once(moved).chain(button)
}

impl Game<'_> {
    fn touch_layout(&self) -> Option<Layout> {
        let r = self.renderer.as_ref()?;
        Some(Layout::new(r.config.width, r.config.height))
    }

    pub(super) fn touch_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, touch: Touch) {
        let Some(layout) = self.touch_layout() else {
            return;
        };
        let at = [touch.location.x, touch.location.y];
        let inputs = self
            .touch
            .touch(&layout, touch.id, phase(touch.phase), at, Instant::now());
        self.apply_touch(event_loop, id, touch.device_id, inputs);
    }

    /// Once a frame, after the pads: what the screen shows, and held fingers.
    pub(super) fn touch_frame(&mut self, event_loop: &ActiveEventLoop) {
        let Some(id) = self.window.as_ref().map(|w| w.id()) else {
            return;
        };
        let mode = if self.title.is_some() {
            Mode::Title
        } else if self.pad_in_menu() {
            Mode::Menu
        } else {
            Mode::Playing
        };
        let inputs = self.touch.frame(mode, Instant::now());
        self.apply_touch(event_loop, id, DeviceId::dummy(), inputs);
    }

    fn apply_touch(
        &mut self,
        event_loop: &ActiveEventLoop,
        id: WindowId,
        device_id: DeviceId,
        inputs: Vec<Input>,
    ) {
        for input in inputs {
            if !matches!(input, Input::Look(_)) {
                self.last_input = Instant::now();
            }
            let result = match input {
                Input::Pointer { at, pressed } => {
                    for event in pointer_events(device_id, at, pressed) {
                        self.window_event(event_loop, id, event);
                    }
                    Ok(())
                }
                Input::Look([dx, dy]) => {
                    self.mouse.add_motion(dx, dy);
                    Ok(())
                }
                Input::Mapping(mapping, down) => self.mapping_action(mapping, down),
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

    pub(super) fn draw_touch(&self, batch: &mut HudBatch) {
        if let Some(layout) = self.touch_layout() {
            let mut ui = horizon_client::ui_theme::Ui::new(&self.gui.atlas, batch, self.cursor);
            self.touch.draw(&mut ui, &layout);
            ui.finish();
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
            other => {
                return Err(format!("touch phase must be start, move or end, not {other}").into());
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
        let Some(r) = &self.renderer else {
            return Ok(());
        };
        let layout = Layout::new(r.config.width, r.config.height);
        let at = [touch.location.x, touch.location.y];
        let inputs = self
            .touch
            .touch(&layout, touch.id, phase(touch.phase), at, Instant::now());
        for input in inputs {
            if let Input::Pointer { at, pressed } = input {
                for event in pointer_events(touch.device_id, at, pressed) {
                    self.menu_event(event_loop, event)?;
                }
            }
        }
        Ok(())
    }
}
