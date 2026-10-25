//! Touch screens: Horizon's own (vanilla has no touch input), on iPhone and iPad. On the title
//! screen and in menus a finger is the mouse pointer, and in a world's menus a Back button does
//! what Escape does. In play the screen carries Bedrock Edition's touch controls: a joystick
//! that appears under the left thumb (pushed all the way forward it sprints), the rest of the
//! screen turning the view, where a tap uses the held item and a finger held still attacks and
//! mines, the hotbar's slots selecting themselves, and buttons to jump, use, sneak (a toggle),
//! open the inventory and pause. Nothing is drawn until a finger first touches the screen.
//!
//! This module is the state machine and the drawing; the game applies its `Input`s.

use std::time::{Duration, Instant};

use crate::hud::HudBatch;
use crate::ui_theme::{self, Face, Ui, color};

/// A finger held still on the view this long attacks.
const HOLD: Duration = Duration::from_millis(300);
/// How far (dp) a finger on the view may drift and still tap or hold.
const SLOP: f32 = 12.0;
/// The joystick's reach (dp): a finger this far from where it landed tilts it all the way.
const STICK_REACH: f32 = 64.0;
/// The joystick's share of the screen, from the left edge.
const STICK_SHARE: f32 = 0.4;
/// Pushed this far forward, the joystick sprints.
const SPRINT_TILT: f32 = 0.9;
/// Mouse movement per physical pixel dragged (`MouseHandler` counts pixels).
const LOOK_SPEED: f64 = 0.5;
/// Button sizes and the margin to the screen's edges (dp).
const BUTTON: f32 = 72.0;
const SMALL_BUTTON: f32 = 56.0;
const MARGIN: f32 = 24.0;
const GAP: f32 = 12.0;
/// The plates' opacity over the world.
const PLATE: f32 = 0.55;
/// Where the pointer goes when its finger lifts: nothing is under a touch screen's pointer, so
/// nothing should stay hovered or show its tooltip.
const AWAY: [f64; 2] = [-1.0, -1.0];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Start,
    Move,
    End,
}

/// What the screen shows, which decides what a finger does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// The title screen and its pages: fingers are the pointer.
    #[default]
    Title,
    /// A menu over a world: the pointer, and Back.
    Menu,
    Playing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Jump,
    Use,
    Sneak,
    Inventory,
    Pause,
    Back,
}

impl Button {
    const IN_PLAY: [Button; 5] = [
        Button::Jump,
        Button::Use,
        Button::Sneak,
        Button::Inventory,
        Button::Pause,
    ];
    fn label(self) -> &'static str {
        match self {
            Button::Jump => "Jump",
            Button::Use => "Use",
            Button::Sneak => "Sneak",
            Button::Inventory => "•••",
            Button::Pause => "II",
            Button::Back => "Back",
        }
    }
    /// The vanilla key mapping it holds down while pressed.
    fn mapping(self) -> Option<&'static str> {
        match self {
            Button::Jump => Some("key.jump"),
            Button::Use => Some("key.use"),
            Button::Inventory => Some("key.inventory"),
            Button::Sneak | Button::Pause | Button::Back => None,
        }
    }
}

/// What a touch does to the game.
#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    /// The mouse pointer at a physical position, pressed or released there.
    Pointer {
        at: [f64; 2],
        pressed: Option<bool>,
    },
    /// Mouse movement turning the view.
    Look([f64; 2]),
    /// A vanilla key mapping pressed or released.
    Mapping(&'static str, bool),
    Hotbar(u8),
    /// What Escape does: pause, or leave a menu.
    Back,
}

/// What a finger does, decided where it lands.
#[derive(Clone, Copy, Debug)]
enum Role {
    Pointer {
        at: [f64; 2],
    },
    /// The joystick, centred where the finger landed (dp).
    Stick {
        origin: [f32; 2],
    },
    Look {
        landed: Instant,
        from: [f64; 2],
        last: [f64; 2],
        still: bool,
        attacking: bool,
    },
    Button(Button),
    Hotbar,
    /// A finger that does nothing until it lifts: a second one on a menu, or one the screen
    /// changed under.
    Spent,
}

#[derive(Clone, Copy, Debug)]
struct Finger {
    id: u64,
    role: Role,
}

/// Where the controls are on a screen of a given size.
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    /// The screen in physical pixels.
    size: [f32; 2],
    /// Physical pixels per GUI pixel (the HUD's) and per dp (the design system's).
    gui: f32,
    dp: f32,
}

impl Layout {
    pub fn new(width: u32, height: u32) -> Self {
        let gui = HudBatch::new([width, height]).scale;
        let size = [width as f32, height as f32];
        Self {
            size,
            gui,
            dp: ui_theme::density(size[0], size[1]),
        }
    }
    fn dp(&self, at: [f64; 2]) -> [f32; 2] {
        [at[0] as f32 / self.dp, at[1] as f32 / self.dp]
    }
    fn screen(&self) -> [f32; 2] {
        [self.size[0] / self.dp, self.size[1] / self.dp]
    }
    /// The hotbar in GUI pixels (`Hud.extractItemHotbar`): left edge and top.
    fn hotbar(&self) -> (f32, f32) {
        let w = (self.size[0] / self.gui).ceil() as i32;
        let h = (self.size[1] / self.gui).ceil() as i32;
        ((w / 2 - 91) as f32, (h - 22) as f32)
    }
    /// A button's rectangle in dp: x, y, width, height.
    fn rect(&self, button: Button) -> [f32; 4] {
        let [w, h] = self.screen();
        let right = w - MARGIN - BUTTON;
        let bottom = h - MARGIN - BUTTON;
        match button {
            Button::Jump => [right, bottom, BUTTON, BUTTON],
            Button::Use => [right, bottom - GAP - BUTTON, BUTTON, BUTTON],
            Button::Sneak => [right - GAP - BUTTON, bottom, BUTTON, BUTTON],
            Button::Inventory => {
                let (x, y) = self.hotbar();
                let side = 22.0 * self.gui / self.dp;
                [
                    (x + 182.0) * self.gui / self.dp + GAP / 2.0,
                    y * self.gui / self.dp,
                    side,
                    side,
                ]
            }
            Button::Pause => [
                w - MARGIN - SMALL_BUTTON,
                MARGIN,
                SMALL_BUTTON,
                SMALL_BUTTON,
            ],
            Button::Back => [MARGIN, MARGIN, SMALL_BUTTON * 1.5, SMALL_BUTTON],
        }
    }
    fn on(&self, button: Button, at: [f32; 2]) -> bool {
        let [x, y, w, h] = self.rect(button);
        (x..x + w).contains(&at[0]) && (y..y + h).contains(&at[1])
    }
    fn hotbar_slot(&self, at: [f64; 2]) -> Option<u8> {
        let (x0, y0) = self.hotbar();
        let (x, y) = (at[0] as f32 / self.gui, at[1] as f32 / self.gui);
        ((x0..x0 + 182.0).contains(&x) && y >= y0)
            .then(|| ((x - x0 - 1.0) / 20.0).floor().clamp(0.0, 8.0) as u8)
    }
    /// Where the joystick rests while no thumb holds it (dp).
    fn stick_home(&self) -> [f32; 2] {
        let [_, h] = self.screen();
        [MARGIN + 2.0 * STICK_REACH, h - MARGIN - 2.0 * STICK_REACH]
    }
}

#[derive(Default)]
pub struct Touch {
    fingers: Vec<Finger>,
    mode: Mode,
    /// The joystick: x right, y forward, each -1..1.
    stick: [f32; 2],
    sprinting: bool,
    sneaking: bool,
    /// A finger has touched the screen: the controls are drawn from then on.
    seen: bool,
}

impl Touch {
    /// The joystick's tilt while a thumb holds it in play.
    pub fn stick(&self) -> Option<[f32; 2]> {
        (self.mode == Mode::Playing
            && self
                .fingers
                .iter()
                .any(|f| matches!(f.role, Role::Stick { .. })))
        .then_some(self.stick)
    }

    /// Fingers steer the view: the game turns it as for a grabbed mouse.
    pub fn steering(&self) -> bool {
        self.seen && self.mode == Mode::Playing
    }

    pub fn touch(
        &mut self,
        layout: &Layout,
        id: u64,
        phase: Phase,
        at: [f64; 2],
        now: Instant,
    ) -> Vec<Input> {
        let mut out = Vec::new();
        let index = self.fingers.iter().position(|f| f.id == id);
        match (phase, index) {
            (Phase::Start, _) => {
                if let Some(i) = index {
                    let finger = self.fingers.remove(i);
                    self.lift(finger.role, at, now, &mut out);
                }
                self.seen = true;
                let role = self.land(layout, at, now, &mut out);
                self.fingers.push(Finger { id, role });
            }
            (Phase::Move, Some(i)) => {
                let mut role = self.fingers[i].role;
                self.slide(layout, &mut role, at, &mut out);
                self.fingers[i].role = role;
            }
            (Phase::End, Some(i)) => {
                let finger = self.fingers.remove(i);
                self.lift(finger.role, at, now, &mut out);
            }
            (Phase::Move | Phase::End, None) => {}
        }
        out
    }

    /// Once a frame: what the screen shows now, and the fingers held long enough to attack.
    pub fn frame(&mut self, mode: Mode, now: Instant) -> Vec<Input> {
        let mut out = Vec::new();
        if mode != self.mode {
            // What fingers held stops with the screen they held it on.
            for i in 0..self.fingers.len() {
                let role = std::mem::replace(&mut self.fingers[i].role, Role::Spent);
                self.release(role, &mut out);
            }
            if self.sneaking {
                out.push(Input::Mapping("key.sneak", mode == Mode::Playing));
            }
            self.mode = mode;
        }
        for finger in &mut self.fingers {
            if let Role::Look {
                landed,
                still: true,
                attacking: attacking @ false,
                ..
            } = &mut finger.role
                && now.duration_since(*landed) >= HOLD
            {
                *attacking = true;
                out.push(Input::Mapping("key.attack", true));
            }
        }
        out
    }

    fn land(&mut self, layout: &Layout, at: [f64; 2], now: Instant, out: &mut Vec<Input>) -> Role {
        let dp = layout.dp(at);
        if self.mode != Mode::Playing {
            if self.mode == Mode::Menu && layout.on(Button::Back, dp) {
                out.push(Input::Back);
                return Role::Button(Button::Back);
            }
            if self
                .fingers
                .iter()
                .any(|f| matches!(f.role, Role::Pointer { .. }))
            {
                return Role::Spent;
            }
            out.push(Input::Pointer {
                at,
                pressed: Some(true),
            });
            return Role::Pointer { at };
        }
        if let Some(button) = Button::IN_PLAY.into_iter().find(|b| layout.on(*b, dp)) {
            match button {
                Button::Sneak => {
                    self.sneaking = !self.sneaking;
                    out.push(Input::Mapping("key.sneak", self.sneaking));
                }
                Button::Pause => out.push(Input::Back),
                _ => out.extend(button.mapping().map(|m| Input::Mapping(m, true))),
            }
            return Role::Button(button);
        }
        if let Some(slot) = layout.hotbar_slot(at) {
            out.push(Input::Hotbar(slot));
            return Role::Hotbar;
        }
        if dp[0] < layout.screen()[0] * STICK_SHARE {
            return Role::Stick { origin: dp };
        }
        Role::Look {
            landed: now,
            from: at,
            last: at,
            still: true,
            attacking: false,
        }
    }

    fn slide(&mut self, layout: &Layout, role: &mut Role, at: [f64; 2], out: &mut Vec<Input>) {
        match role {
            Role::Pointer { at: last } => {
                *last = at;
                out.push(Input::Pointer { at, pressed: None });
            }
            Role::Stick { origin } => {
                let dp = layout.dp(at);
                let (x, y) = (
                    (dp[0] - origin[0]) / STICK_REACH,
                    (origin[1] - dp[1]) / STICK_REACH,
                );
                let length = x.hypot(y).max(1.0);
                self.stick = [x / length, y / length];
                let sprint = self.stick[1] >= SPRINT_TILT;
                if sprint != self.sprinting {
                    self.sprinting = sprint;
                    out.push(Input::Mapping("key.sprint", sprint));
                }
            }
            Role::Look {
                from, last, still, ..
            } => {
                out.push(Input::Look([
                    (at[0] - last[0]) * LOOK_SPEED,
                    (at[1] - last[1]) * LOOK_SPEED,
                ]));
                *last = at;
                let (start, now) = (layout.dp(*from), layout.dp(at));
                if (now[0] - start[0]).hypot(now[1] - start[1]) > SLOP {
                    *still = false;
                }
            }
            Role::Hotbar => out.extend(layout.hotbar_slot(at).map(Input::Hotbar)),
            Role::Button(_) | Role::Spent => {}
        }
    }

    fn lift(&mut self, role: Role, at: [f64; 2], now: Instant, out: &mut Vec<Input>) {
        match role {
            Role::Pointer { .. } => Self::lift_pointer(at, out),
            // A tap uses the held item: a click of the right button.
            Role::Look {
                landed,
                still: true,
                attacking: false,
                ..
            } if now.duration_since(landed) < HOLD => {
                out.push(Input::Mapping("key.use", true));
                out.push(Input::Mapping("key.use", false));
            }
            role => self.release(role, out),
        }
    }

    fn lift_pointer(at: [f64; 2], out: &mut Vec<Input>) {
        out.push(Input::Pointer {
            at,
            pressed: Some(false),
        });
        out.push(Input::Pointer {
            at: AWAY,
            pressed: None,
        });
    }

    /// Let go of what a finger holds down.
    fn release(&mut self, role: Role, out: &mut Vec<Input>) {
        match role {
            Role::Pointer { at } => Self::lift_pointer(at, out),
            Role::Stick { .. } => {
                self.stick = [0.0, 0.0];
                if std::mem::take(&mut self.sprinting) {
                    out.push(Input::Mapping("key.sprint", false));
                }
            }
            Role::Look {
                attacking: true, ..
            } => out.push(Input::Mapping("key.attack", false)),
            Role::Button(button) => {
                out.extend(button.mapping().map(|m| Input::Mapping(m, false)));
            }
            Role::Look { .. } | Role::Hotbar | Role::Spent => {}
        }
    }

    pub fn draw(&self, ui: &mut Ui, layout: &Layout) {
        if !self.seen {
            return;
        }
        match self.mode {
            Mode::Title => {}
            Mode::Menu => self.draw_button(ui, layout, Button::Back),
            Mode::Playing => {
                let origin = self.fingers.iter().find_map(|f| match f.role {
                    Role::Stick { origin } => Some(origin),
                    _ => None,
                });
                let [cx, cy] = origin.unwrap_or_else(|| layout.stick_home());
                let r = STICK_REACH;
                ui.rounded(
                    cx - r,
                    cy - r,
                    2.0 * r,
                    2.0 * r,
                    r,
                    color::with_alpha(color::BG, PLATE),
                );
                ui.border(cx - r, cy - r, 2.0 * r, 2.0 * r, r, color::BORDER);
                let knob = r * 0.45;
                let (kx, ky) = (cx + self.stick[0] * r, cy - self.stick[1] * r);
                ui.rounded(
                    kx - knob,
                    ky - knob,
                    2.0 * knob,
                    2.0 * knob,
                    knob,
                    color::TEXT,
                );
                for button in Button::IN_PLAY {
                    self.draw_button(ui, layout, button);
                }
            }
        }
    }

    fn draw_button(&self, ui: &mut Ui, layout: &Layout, button: Button) {
        let [x, y, w, h] = layout.rect(button);
        let held = self
            .fingers
            .iter()
            .any(|f| matches!(f.role, Role::Button(b) if b == button));
        let (plate, ink) = if held || (button == Button::Sneak && self.sneaking) {
            (color::ACCENT, color::ON_ACCENT)
        } else {
            (color::with_alpha(color::BG, PLATE), color::TEXT)
        };
        ui.rounded(x, y, w, h, ui_theme::RADIUS_PLATE, plate);
        ui.border(x, y, w, h, ui_theme::RADIUS_PLATE, color::BORDER);
        let line = (ui_theme::BODY * 1.45).round();
        ui.text_centered(
            Face::Medium,
            ui_theme::BODY,
            x + w / 2.0,
            y + (h - line) / 2.0,
            ink,
            button.label(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An iPhone in landscape: GUI scale 5, 1.72 px per dp.
    fn layout() -> Layout {
        Layout::new(2622, 1206)
    }

    fn playing() -> (Touch, Layout, Instant) {
        let mut touch = Touch::default();
        let now = Instant::now();
        assert!(touch.frame(Mode::Playing, now).is_empty());
        (touch, layout(), now)
    }

    fn px(layout: &Layout, dp: [f32; 2]) -> [f64; 2] {
        [f64::from(dp[0] * layout.dp), f64::from(dp[1] * layout.dp)]
    }

    fn centre(layout: &Layout, button: Button) -> [f64; 2] {
        let [x, y, w, h] = layout.rect(button);
        px(layout, [x + w / 2.0, y + h / 2.0])
    }

    #[test]
    fn a_finger_on_a_menu_is_the_pointer_and_a_second_one_does_nothing() {
        let (mut touch, layout) = (Touch::default(), layout());
        let now = Instant::now();
        let press = touch.touch(&layout, 1, Phase::Start, [100.0, 200.0], now);
        assert_eq!(
            press,
            [Input::Pointer {
                at: [100.0, 200.0],
                pressed: Some(true)
            }]
        );
        assert!(
            touch
                .touch(&layout, 2, Phase::Start, [900.0, 200.0], now)
                .is_empty()
        );
        assert_eq!(
            touch.touch(&layout, 1, Phase::Move, [120.0, 210.0], now),
            [Input::Pointer {
                at: [120.0, 210.0],
                pressed: None
            }]
        );
        assert_eq!(
            touch.touch(&layout, 1, Phase::End, [120.0, 210.0], now),
            [
                Input::Pointer {
                    at: [120.0, 210.0],
                    pressed: Some(false)
                },
                Input::Pointer {
                    at: AWAY,
                    pressed: None
                }
            ]
        );
        assert!(
            touch
                .touch(&layout, 2, Phase::End, [900.0, 200.0], now)
                .is_empty()
        );
    }

    #[test]
    fn back_leaves_a_world_menu_but_not_the_title() {
        let (mut touch, layout) = (Touch::default(), layout());
        let now = Instant::now();
        let back = centre(&layout, Button::Back);
        assert!(matches!(
            touch.touch(&layout, 1, Phase::Start, back, now)[..],
            [Input::Pointer { .. }]
        ));
        touch.touch(&layout, 1, Phase::End, back, now);
        touch.frame(Mode::Menu, now);
        assert_eq!(
            touch.touch(&layout, 2, Phase::Start, back, now),
            [Input::Back]
        );
    }

    #[test]
    fn a_tap_on_the_view_uses_and_a_held_finger_attacks_until_lifted() {
        let (mut touch, layout, now) = playing();
        let view = [2000.0, 500.0];
        assert!(touch.touch(&layout, 1, Phase::Start, view, now).is_empty());
        assert_eq!(
            touch.touch(&layout, 1, Phase::End, view, now + HOLD / 2),
            [
                Input::Mapping("key.use", true),
                Input::Mapping("key.use", false)
            ]
        );

        touch.touch(&layout, 2, Phase::Start, view, now);
        assert!(touch.frame(Mode::Playing, now + HOLD / 2).is_empty());
        assert_eq!(
            touch.frame(Mode::Playing, now + HOLD),
            [Input::Mapping("key.attack", true)]
        );
        assert!(touch.frame(Mode::Playing, now + HOLD * 2).is_empty());
        assert_eq!(
            touch.touch(&layout, 2, Phase::End, view, now + HOLD * 3),
            [Input::Mapping("key.attack", false)]
        );
    }

    #[test]
    fn dragging_the_view_turns_it_and_neither_uses_nor_attacks() {
        let (mut touch, layout, now) = playing();
        touch.touch(&layout, 1, Phase::Start, [2000.0, 500.0], now);
        assert_eq!(
            touch.touch(&layout, 1, Phase::Move, [2100.0, 460.0], now),
            [Input::Look([50.0, -20.0])]
        );
        assert!(touch.frame(Mode::Playing, now + HOLD * 2).is_empty());
        assert!(
            touch
                .touch(&layout, 1, Phase::End, [2100.0, 460.0], now + HOLD * 2)
                .is_empty()
        );
    }

    #[test]
    fn the_joystick_tilts_from_where_the_thumb_landed_and_sprints_pushed_forward() {
        let (mut touch, layout, now) = playing();
        let landed = [300.0, 500.0];
        touch.touch(&layout, 1, Phase::Start, px(&layout, landed), now);
        assert_eq!(touch.stick(), Some([0.0, 0.0]));
        let half_right = px(&layout, [landed[0] + STICK_REACH / 2.0, landed[1]]);
        assert!(
            touch
                .touch(&layout, 1, Phase::Move, half_right, now)
                .is_empty()
        );
        let [x, y] = touch.stick().unwrap();
        assert!((x - 0.5).abs() < 1e-3 && y.abs() < 1e-3, "{x} {y}");
        let far_forward = px(&layout, [landed[0], landed[1] - 3.0 * STICK_REACH]);
        assert_eq!(
            touch.touch(&layout, 1, Phase::Move, far_forward, now),
            [Input::Mapping("key.sprint", true)]
        );
        let [x, y] = touch.stick().unwrap();
        assert!(x.abs() < 1e-3 && (y - 1.0).abs() < 1e-3, "{x} {y}");
        assert_eq!(
            touch.touch(&layout, 1, Phase::End, far_forward, now),
            [Input::Mapping("key.sprint", false)]
        );
        assert_eq!(touch.stick(), None);
    }

    #[test]
    fn buttons_hold_their_mapping_and_sneak_toggles() {
        let (mut touch, layout, now) = playing();
        let jump = centre(&layout, Button::Jump);
        assert_eq!(
            touch.touch(&layout, 1, Phase::Start, jump, now),
            [Input::Mapping("key.jump", true)]
        );
        assert_eq!(
            touch.touch(&layout, 1, Phase::End, jump, now),
            [Input::Mapping("key.jump", false)]
        );
        let sneak = centre(&layout, Button::Sneak);
        assert_eq!(
            touch.touch(&layout, 2, Phase::Start, sneak, now),
            [Input::Mapping("key.sneak", true)]
        );
        assert!(touch.touch(&layout, 2, Phase::End, sneak, now).is_empty());
        assert_eq!(
            touch.touch(&layout, 3, Phase::Start, sneak, now),
            [Input::Mapping("key.sneak", false)]
        );
        let pause = centre(&layout, Button::Pause);
        assert_eq!(
            touch.touch(&layout, 4, Phase::Start, pause, now),
            [Input::Back]
        );
    }

    #[test]
    fn the_hotbar_selects_the_slot_under_the_finger() {
        let (mut touch, layout, now) = playing();
        // GUI 525 x 242: the hotbar spans x 171..353 above y 220.
        let slot = |i: f64| [(171.0 + 1.0 + 20.0 * i + 10.0) * 5.0, 230.0 * 5.0];
        assert_eq!(
            touch.touch(&layout, 1, Phase::Start, slot(0.0), now),
            [Input::Hotbar(0)]
        );
        assert_eq!(
            touch.touch(&layout, 1, Phase::Move, slot(8.0), now),
            [Input::Hotbar(8)]
        );
    }

    #[test]
    fn leaving_play_lets_go_of_everything_held() {
        let (mut touch, layout, now) = playing();
        touch.touch(&layout, 1, Phase::Start, centre(&layout, Button::Jump), now);
        touch.touch(&layout, 2, Phase::Start, [2000.0, 500.0], now);
        touch.frame(Mode::Playing, now + HOLD);
        touch.touch(&layout, 3, Phase::Start, px(&layout, [300.0, 500.0]), now);
        let released = touch.frame(Mode::Menu, now + HOLD);
        assert_eq!(
            released,
            [
                Input::Mapping("key.jump", false),
                Input::Mapping("key.attack", false)
            ]
        );
        assert_eq!(touch.stick(), None);
        assert!(
            touch
                .touch(&layout, 2, Phase::End, [2000.0, 500.0], now)
                .is_empty()
        );
    }
}
