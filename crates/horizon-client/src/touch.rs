//! Touch screens: Horizon's own (vanilla has no touch input), on iPhone and iPad, with Pocket
//! Edition's controls. On the title screen and in menus a finger is the mouse, its button
//! waiting for the gesture (`menu`), and in a world's menus a Back button does what Escape does.
//! In play:
//!
//! - the D-pad moves (forward, back, the sides, and the forward diagonals while forward is
//!   held), a double tap on forward sprints, and its middle sneaks (a toggle) or, while flying,
//!   goes down; the joystick can take its place (`Movement`);
//! - Jump jumps (a double tap flies in creative) and goes up while flying;
//! - the rest of the screen turns the view: a tap attacks the mob under the crosshair or uses
//!   the held item, and a finger held still mines, or eats and draws a bow when the held item is
//!   used over time;
//! - the hotbar selects its slots, the button beside it opens the inventory, and the one in the
//!   top corner pauses.
//!
//! Nothing is drawn until a finger first touches the screen. This module is the state machine;
//! `layout` places the controls, `draw` draws them, and the game applies the `Input`s.

use std::time::{Duration, Instant};

mod draw;
mod keys;
mod layout;
mod menu;
use keys::Keys;
pub use layout::{Button, Cell, Layout};
pub use menu::{Here, MouseButton, SlotKind, SlotSpot, Spot};

/// A finger held still on the view this long mines, or uses the held item; on a menu it is the
/// right button.
const HOLD: Duration = Duration::from_millis(300);
/// Forward pressed again this soon after it was let go sprints.
const DOUBLE_TAP: Duration = Duration::from_millis(300);
/// Pushed this far forward, the joystick sprints.
const SPRINT_TILT: f32 = 0.9;
/// The joystick's middle that doesn't move yet, as a share of its reach.
const DEADZONE: f32 = 0.15;
/// Mouse movement per physical pixel dragged (`MouseHandler` counts pixels).
const LOOK_SPEED: f64 = 0.5;
/// Where the pointer goes when its finger lifts: nothing is under a touch screen's pointer, so
/// nothing should stay hovered or show its tooltip.
pub const AWAY: [f64; 2] = [-1.0, -1.0];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Start,
    Move,
    End,
    /// The system took the touch (a call, a system gesture): let go without tapping.
    Cancel,
}

/// One touch as the screen reports it, and what the game shows under it.
#[derive(Clone, Copy, Debug)]
pub struct TouchEvent {
    pub id: u64,
    pub phase: Phase,
    /// Physical pixels.
    pub at: [f64; 2],
    pub here: Here,
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

/// What moves the player: Pocket Edition's D-pad, or a joystick (`touchMovement` in
/// `horizon-controller.txt`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Movement {
    #[default]
    DPad,
    Joystick,
}

impl Movement {
    pub fn name(self) -> &'static str {
        match self {
            Movement::DPad => "dpad",
            Movement::Joystick => "joystick",
        }
    }
    pub fn parse(name: &str) -> Self {
        match name {
            "joystick" => Movement::Joystick,
            _ => Movement::DPad,
        }
    }
}

/// What the game tells the controls each frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Context {
    pub mode: Mode,
    pub movement: Movement,
    /// The player is flying: Jump goes up and the D-pad's middle down.
    pub flying: bool,
    /// The held item is used over time (food, a bow, a shield...): a held finger uses it.
    pub held_item_used_over_time: bool,
}

/// What a touch does to the game. On the title screen and in menus, only what a desk's mouse
/// and keyboard would send, so nothing reaches a menu another way.
#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    /// The mouse pointer moves to a physical position: what is there is hovered.
    Pointer([f64; 2]),
    /// A mouse button goes down where the pointer is. `fresh`: a press of the touch layer's own
    /// (a finger leaving where it landed), never the second click of a double click.
    Press {
        button: MouseButton,
        fresh: bool,
    },
    /// The mouse button goes up where the pointer is.
    Release(MouseButton),
    /// The mouse button goes up, and its release must click nothing: the touch was cancelled, or
    /// the screen changed under it.
    Abort(MouseButton),
    /// If the hand holds a stack now, a left click here (physical pixels): a dropped stack's
    /// swap or remainder goes back where the stack came from.
    PutBack([f64; 2]),
    /// A tap focused a text field: the system keyboard is wanted.
    Keyboard,
    /// Mouse movement turning the view.
    Look([f64; 2]),
    /// A vanilla key mapping pressed or released.
    Mapping(&'static str, bool),
    /// Sprinting wanted, or no longer: a state, not a key edge.
    Sprint(bool),
    /// The D-pad's middle: sneaking flips.
    Sneak,
    /// A quick still tap on the view: an attack on the mob under the crosshair, else a use.
    Tap,
    Hotbar(u8),
    /// What Escape does: pause, or leave a menu.
    Back,
}

/// What a finger does, decided where it lands.
#[derive(Clone, Copy, Debug)]
enum Role {
    /// On the title screen or a menu.
    Menu(menu::Gesture),
    /// On the D-pad: the square under it, which it may slide to another.
    Pad {
        cell: Option<Cell>,
    },
    /// The joystick, centred where the finger landed.
    Stick {
        origin: [f32; 2],
    },
    Look {
        landed: Instant,
        from: [f64; 2],
        last: [f64; 2],
        still: bool,
        /// What holding still pressed: `key.attack`, or `key.use`.
        holding: Option<&'static str>,
    },
    Button(Button),
    /// The D-pad's middle while flying: down.
    Descend,
    Hotbar,
    /// A finger that does nothing until it lifts: a second one on a menu or on the D-pad, the
    /// middle's tap, or one the screen changed under.
    Spent,
}

impl Role {
    /// The key mapping this finger holds down.
    fn holds(&self) -> Option<&'static str> {
        match *self {
            Role::Button(Button::Jump) => Some("key.jump"),
            Role::Button(Button::Inventory) => Some("key.inventory"),
            Role::Descend => Some("key.sneak"),
            Role::Look { holding, .. } => holding,
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Finger {
    id: u64,
    role: Role,
}

#[derive(Default)]
pub struct Touch {
    fingers: Vec<Finger>,
    context: Context,
    /// Where the D-pad or the joystick moves the player: x right, y forward, each -1..1.
    stick: [f32; 2],
    sprinting: bool,
    /// When forward was last let go, for the double tap that sprints.
    forward_released: Option<Instant>,
    keys: Keys,
    /// A finger has touched the screen: the controls are drawn from then on.
    seen: bool,
    menus: menu::Menus,
}

impl Touch {
    /// The move vector while a thumb is on the D-pad or the joystick in play, as the game takes
    /// it (`KeyboardInput`'s `[left, forward]`, as `gamepad::move_vector` gives the pad's).
    pub fn move_vector(&self) -> Option<[f32; 2]> {
        let [right, forward] = self.stick;
        (self.context.mode == Mode::Playing && self.moving()).then_some([-right, forward])
    }

    fn moving(&self) -> bool {
        self.fingers
            .iter()
            .any(|f| matches!(f.role, Role::Pad { .. } | Role::Stick { .. }))
    }

    /// Fingers steer the view: the game turns it as for a grabbed mouse.
    pub fn steering(&self) -> bool {
        self.seen && self.context.mode == Mode::Playing
    }

    /// A finger is on the screen: the player is playing, not away.
    pub fn active(&self) -> bool {
        !self.fingers.is_empty()
    }

    pub fn touch(&mut self, layout: &Layout, event: TouchEvent, now: Instant) -> Vec<Input> {
        let TouchEvent {
            id,
            phase,
            at,
            here,
        } = event;
        let mut out = Vec::new();
        let index = self.fingers.iter().position(|f| f.id == id);
        match (phase, index) {
            (Phase::Start, _) => {
                if let Some(i) = index {
                    let finger = self.fingers.remove(i);
                    self.release(finger.role, now, &mut out);
                }
                self.seen = true;
                let role = if self.context.mode == Mode::Playing {
                    self.land(layout, at, now, &mut out)
                } else {
                    self.land_menu(layout, at, here, now, &mut out)
                };
                self.fingers.push(Finger { id, role });
            }
            (Phase::Move, Some(i)) => {
                let mut role = self.fingers[i].role;
                match &mut role {
                    Role::Menu(gesture) => self.slide_menu(layout, gesture, at, here, &mut out),
                    role => self.slide(layout, role, at, now, &mut out),
                }
                self.fingers[i].role = role;
            }
            (Phase::End, Some(i)) => {
                let finger = self.fingers.remove(i);
                match finger.role {
                    Role::Menu(gesture) => self.lift_menu(gesture, at, here, &mut out),
                    role => self.lift(role, now, &mut out),
                }
            }
            (Phase::Cancel, Some(i)) => {
                let finger = self.fingers.remove(i);
                self.release(finger.role, now, &mut out);
            }
            (Phase::Move | Phase::End | Phase::Cancel, None) => {}
        }
        out
    }

    /// Once a frame: what the game shows now, the key edges whose time has come, and the
    /// fingers held still long enough to mine or use.
    pub fn frame(&mut self, context: Context, now: Instant) -> Vec<Input> {
        let mut out = Vec::new();
        let mode_changed = context.mode != self.context.mode;
        self.context = context;
        if mode_changed {
            // What fingers held stops with the screen they held it on.
            for i in 0..self.fingers.len() {
                let role = std::mem::replace(&mut self.fingers[i].role, Role::Spent);
                match role {
                    Role::Menu(gesture) => self.let_go(gesture, &mut out),
                    Role::Pad { .. } | Role::Stick { .. } => self.stop_moving(now, &mut out),
                    _ => {}
                }
            }
            self.keys.release_all(now, &mut out);
        }
        self.keys.due(now, &mut out);
        self.menu_holds(now, &mut out);
        let mapping = if context.held_item_used_over_time {
            "key.use"
        } else {
            "key.attack"
        };
        for finger in &mut self.fingers {
            if let Role::Look {
                landed,
                still: true,
                holding: holding @ None,
                ..
            } = &mut finger.role
                && now.duration_since(*landed) >= HOLD
            {
                *holding = Some(mapping);
                self.keys.set(mapping, true, now, &mut out);
            }
        }
        out
    }

    /// A finger holds `mapping` down.
    fn fingers_hold(&self, mapping: &str) -> bool {
        self.fingers.iter().any(|f| f.role.holds() == Some(mapping))
    }

    fn land(&mut self, layout: &Layout, at: [f64; 2], now: Instant, out: &mut Vec<Input>) -> Role {
        let px = [at[0] as f32, at[1] as f32];
        for button in [Button::Jump, Button::Inventory, Button::Pause] {
            if layout.on(button, px) {
                let role = Role::Button(button);
                match role.holds() {
                    Some(mapping) => self.keys.set(mapping, true, now, out),
                    None => out.push(Input::Back),
                }
                return role;
            }
        }
        if let Some(slot) = layout.hotbar_slot(px) {
            out.push(Input::Hotbar(slot));
            return Role::Hotbar;
        }
        let moving = self.moving();
        match self.context.movement {
            Movement::DPad => match layout.cell_at(px) {
                Some(Cell::Middle) if self.context.flying => {
                    self.keys.set("key.sneak", true, now, out);
                    return Role::Descend;
                }
                Some(Cell::Middle) => {
                    out.push(Input::Sneak);
                    return Role::Spent;
                }
                Some(cell) if !moving => {
                    self.step(Some(cell), now, out);
                    return Role::Pad { cell: Some(cell) };
                }
                // A second thumb on the D-pad does nothing.
                Some(_) => return Role::Spent,
                None => {}
            },
            Movement::Joystick if layout.stick_zone(px) && !moving => {
                return Role::Stick { origin: px };
            }
            Movement::Joystick => {}
        }
        Role::Look {
            landed: now,
            from: at,
            last: at,
            still: true,
            holding: None,
        }
    }

    /// The D-pad's thumb moves onto `cell` (none: off the arrows): the move vector, and
    /// sprinting on a double tap of forward.
    fn step(&mut self, cell: Option<Cell>, now: Instant, out: &mut Vec<Input>) {
        let was_forward = self.stick[1] > 0.5;
        let forward = cell.is_some_and(Cell::forward);
        self.stick = cell.map_or([0.0, 0.0], Cell::vector);
        if forward && !was_forward {
            let double = self
                .forward_released
                .is_some_and(|at| now.duration_since(at) <= DOUBLE_TAP);
            if double && !self.sprinting {
                self.sprinting = true;
                out.push(Input::Sprint(true));
            }
        } else if was_forward && !forward {
            self.forward_released = Some(now);
            if std::mem::take(&mut self.sprinting) {
                out.push(Input::Sprint(false));
            }
        }
    }

    fn slide(
        &mut self,
        layout: &Layout,
        role: &mut Role,
        at: [f64; 2],
        now: Instant,
        out: &mut Vec<Input>,
    ) {
        let px = [at[0] as f32, at[1] as f32];
        match role {
            Role::Pad { cell } => {
                // The middle stops the player but doesn't sneak under a sliding thumb.
                let under = layout.cell_at(px).filter(|c| *c != Cell::Middle);
                if under != *cell {
                    *cell = under;
                    self.step(under, now, out);
                }
            }
            Role::Stick { origin } => {
                let reach = layout.reach();
                let (x, y) = ((px[0] - origin[0]) / reach, (origin[1] - px[1]) / reach);
                let length = x.hypot(y);
                self.stick = if length < DEADZONE {
                    [0.0, 0.0]
                } else {
                    let scale = ((length - DEADZONE) / (1.0 - DEADZONE)).min(1.0) / length;
                    [x * scale, y * scale]
                };
                let sprint = self.stick[1] >= SPRINT_TILT;
                if sprint != self.sprinting {
                    self.sprinting = sprint;
                    out.push(Input::Sprint(sprint));
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
                let moved = ((at[0] - from[0]) as f32).hypot((at[1] - from[1]) as f32);
                if moved > layout.slop() {
                    *still = false;
                }
            }
            Role::Hotbar => out.extend(layout.hotbar_slot(px).map(Input::Hotbar)),
            Role::Menu(_) | Role::Button(_) | Role::Descend | Role::Spent => {}
        }
    }

    /// A finger leaves the screen where it was.
    fn lift(&mut self, role: Role, now: Instant, out: &mut Vec<Input>) {
        match role {
            // A tap clicks use or attack, which another finger may be holding down.
            Role::Look {
                landed,
                still: true,
                holding: None,
                ..
            } if now.duration_since(landed) < HOLD
                && !self.keys.held("key.use")
                && !self.keys.held("key.attack") =>
            {
                out.push(Input::Tap);
            }
            role => self.release(role, now, out),
        }
    }

    fn stop_moving(&mut self, now: Instant, out: &mut Vec<Input>) {
        if self.stick[1] > 0.5 {
            self.forward_released = Some(now);
        }
        self.stick = [0.0, 0.0];
        if std::mem::take(&mut self.sprinting) {
            out.push(Input::Sprint(false));
        }
    }

    /// Let go of what a lifted finger held down, once no other finger holds it.
    fn release(&mut self, role: Role, now: Instant, out: &mut Vec<Input>) {
        match role {
            Role::Menu(gesture) => self.let_go(gesture, out),
            Role::Pad { .. } | Role::Stick { .. } => self.stop_moving(now, out),
            role => {
                if let Some(mapping) = role.holds().filter(|m| !self.fingers_hold(m)) {
                    self.keys.set(mapping, false, now, out);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::keys::MIN_STATE;
    use super::*;

    /// A finger with nothing of the game's under it, as in play.
    trait Finger {
        fn finger(
            &mut self,
            layout: &Layout,
            id: u64,
            phase: Phase,
            at: [f64; 2],
            now: Instant,
        ) -> Vec<Input>;
    }

    impl Finger for Touch {
        fn finger(
            &mut self,
            layout: &Layout,
            id: u64,
            phase: Phase,
            at: [f64; 2],
            now: Instant,
        ) -> Vec<Input> {
            let here = Here::default();
            self.touch(
                layout,
                TouchEvent {
                    id,
                    phase,
                    at,
                    here,
                },
                now,
            )
        }
    }

    /// An iPhone in landscape: 3 pixels a point, GUI scale 5, 8-pixel art.
    fn layout() -> Layout {
        Layout::new(2622, 1206, 3.0)
    }

    fn playing(movement: Movement) -> (Touch, Layout, Instant) {
        let mut touch = Touch::default();
        let now = Instant::now();
        let context = Context {
            mode: Mode::Playing,
            movement,
            ..Context::default()
        };
        assert!(touch.frame(context, now).is_empty());
        (touch, layout(), now)
    }

    fn centre(r: layout::Rect) -> [f64; 2] {
        [f64::from(r[0] + r[2] / 2.0), f64::from(r[1] + r[3] / 2.0)]
    }

    fn cell(layout: &Layout, cell: Cell) -> [f64; 2] {
        centre(layout.cell(cell))
    }

    const VIEW: [f64; 2] = [2000.0, 500.0];

    #[test]
    fn a_finger_on_a_menu_is_the_pointer_and_a_second_one_does_nothing() {
        let (mut touch, layout) = (Touch::default(), layout());
        let now = Instant::now();
        assert_eq!(
            touch.finger(&layout, 1, Phase::Start, [100.0, 200.0], now),
            [Input::Pointer([100.0, 200.0])]
        );
        assert!(
            touch
                .finger(&layout, 2, Phase::Start, [900.0, 200.0], now)
                .is_empty()
        );
        assert_eq!(
            touch.finger(&layout, 1, Phase::Move, [120.0, 210.0], now),
            [Input::Pointer([120.0, 210.0])]
        );
        assert_eq!(
            touch.finger(&layout, 1, Phase::End, [120.0, 210.0], now),
            [
                Input::Pointer([120.0, 210.0]),
                Input::Press {
                    button: MouseButton::Left,
                    fresh: false
                },
                Input::Release(MouseButton::Left),
                Input::Pointer(AWAY)
            ]
        );
    }

    #[test]
    fn back_leaves_a_world_menu_but_not_the_title() {
        let (mut touch, layout) = (Touch::default(), layout());
        let now = Instant::now();
        let back = centre(layout.button(Button::Back));
        assert!(matches!(
            touch.finger(&layout, 1, Phase::Start, back, now)[..],
            [Input::Pointer(_)]
        ));
        touch.finger(&layout, 1, Phase::End, back, now);
        let menu = Context {
            mode: Mode::Menu,
            ..Context::default()
        };
        touch.frame(menu, now);
        assert_eq!(
            touch.finger(&layout, 2, Phase::Start, back, now),
            [Input::Back]
        );
    }

    #[test]
    fn the_dpad_moves_while_a_thumb_slides_over_its_arrows() {
        let (mut touch, layout, now) = playing(Movement::DPad);
        let on = |c| cell(&layout, c);
        assert!(
            touch
                .finger(&layout, 1, Phase::Start, on(Cell::Forward), now)
                .is_empty()
        );
        assert_eq!(touch.move_vector(), Some([0.0, 1.0]));
        touch.finger(&layout, 1, Phase::Move, on(Cell::ForwardLeft), now);
        let [left, forward] = touch.move_vector().unwrap();
        assert!(left > 0.7 && forward > 0.7, "{left} {forward}");
        touch.finger(&layout, 1, Phase::Move, on(Cell::Left), now);
        assert_eq!(touch.move_vector(), Some([1.0, 0.0]));
        touch.finger(&layout, 1, Phase::Move, on(Cell::Right), now);
        assert_eq!(touch.move_vector(), Some([-1.0, 0.0]));
        // Over the middle the player stops, without sneaking.
        assert!(
            touch
                .finger(&layout, 1, Phase::Move, on(Cell::Middle), now)
                .is_empty()
        );
        assert_eq!(touch.move_vector(), Some([0.0, 0.0]));
        touch.finger(&layout, 1, Phase::End, on(Cell::Middle), now);
        assert_eq!(touch.move_vector(), None);
    }

    #[test]
    fn a_double_tap_on_forward_sprints_until_forward_is_let_go() {
        let (mut touch, layout, now) = playing(Movement::DPad);
        let ms = |n: u64| now + Duration::from_millis(n);
        let on = |c| cell(&layout, c);
        let forward = on(Cell::Forward);
        assert!(
            touch
                .finger(&layout, 1, Phase::Start, forward, now)
                .is_empty()
        );
        assert!(
            touch
                .finger(&layout, 1, Phase::End, forward, ms(100))
                .is_empty()
        );
        assert_eq!(
            touch.finger(&layout, 2, Phase::Start, forward, ms(250)),
            [Input::Sprint(true)]
        );
        // Sliding to a forward diagonal keeps sprinting; off forward stops it.
        assert!(
            touch
                .finger(&layout, 2, Phase::Move, on(Cell::ForwardRight), ms(300))
                .is_empty()
        );
        assert_eq!(
            touch.finger(&layout, 2, Phase::Move, on(Cell::Right), ms(350)),
            [Input::Sprint(false)]
        );
        touch.finger(&layout, 2, Phase::End, on(Cell::Right), ms(400));
        // Too slow a second tap only walks.
        touch.finger(&layout, 3, Phase::Start, forward, ms(500));
        touch.finger(&layout, 3, Phase::End, forward, ms(550));
        assert!(
            touch
                .finger(&layout, 4, Phase::Start, forward, ms(900))
                .is_empty()
        );
    }

    #[test]
    fn the_middle_sneaks_and_goes_down_while_flying() {
        let (mut touch, layout, now) = playing(Movement::DPad);
        let middle = cell(&layout, Cell::Middle);
        assert_eq!(
            touch.finger(&layout, 1, Phase::Start, middle, now),
            [Input::Sneak]
        );
        assert!(touch.finger(&layout, 1, Phase::End, middle, now).is_empty());
        let flying = Context {
            flying: true,
            ..touch.context
        };
        touch.frame(flying, now);
        assert_eq!(
            touch.finger(&layout, 2, Phase::Start, middle, now),
            [Input::Mapping("key.sneak", true)]
        );
        assert!(touch.finger(&layout, 2, Phase::End, middle, now).is_empty());
        assert_eq!(
            touch.frame(flying, now + MIN_STATE),
            [Input::Mapping("key.sneak", false)]
        );
    }

    #[test]
    fn a_tap_on_the_view_taps_and_a_held_finger_mines_until_lifted() {
        let (mut touch, layout, now) = playing(Movement::DPad);
        assert!(touch.finger(&layout, 1, Phase::Start, VIEW, now).is_empty());
        assert_eq!(
            touch.finger(&layout, 1, Phase::End, VIEW, now + HOLD / 2),
            [Input::Tap]
        );
        touch.finger(&layout, 2, Phase::Start, VIEW, now);
        let context = touch.context;
        assert!(touch.frame(context, now + HOLD / 2).is_empty());
        assert_eq!(
            touch.frame(context, now + HOLD),
            [Input::Mapping("key.attack", true)]
        );
        assert_eq!(
            touch.finger(&layout, 2, Phase::End, VIEW, now + HOLD * 3),
            [Input::Mapping("key.attack", false)]
        );
    }

    #[test]
    fn a_held_finger_uses_an_item_used_over_time() {
        let (mut touch, layout, now) = playing(Movement::DPad);
        let food = Context {
            held_item_used_over_time: true,
            ..touch.context
        };
        touch.frame(food, now);
        touch.finger(&layout, 1, Phase::Start, VIEW, now);
        assert_eq!(
            touch.frame(food, now + HOLD),
            [Input::Mapping("key.use", true)]
        );
        assert_eq!(
            touch.finger(&layout, 1, Phase::End, VIEW, now + HOLD * 4),
            [Input::Mapping("key.use", false)]
        );
    }

    #[test]
    fn a_cancelled_touch_lets_go_without_tapping() {
        let (mut touch, layout, now) = playing(Movement::DPad);
        touch.finger(&layout, 1, Phase::Start, VIEW, now);
        assert!(
            touch
                .finger(&layout, 1, Phase::Cancel, VIEW, now)
                .is_empty()
        );
        touch.finger(&layout, 2, Phase::Start, VIEW, now);
        let context = touch.context;
        touch.frame(context, now + HOLD);
        assert_eq!(
            touch.finger(&layout, 2, Phase::Cancel, VIEW, now + HOLD + MIN_STATE),
            [Input::Mapping("key.attack", false)]
        );
    }

    #[test]
    fn dragging_the_view_turns_it_and_neither_taps_nor_mines() {
        let (mut touch, layout, now) = playing(Movement::DPad);
        touch.finger(&layout, 1, Phase::Start, VIEW, now);
        assert_eq!(
            touch.finger(&layout, 1, Phase::Move, [2100.0, 460.0], now),
            [Input::Look([50.0, -20.0])]
        );
        let context = touch.context;
        assert!(touch.frame(context, now + HOLD * 2).is_empty());
        assert!(
            touch
                .finger(&layout, 1, Phase::End, [2100.0, 460.0], now + HOLD * 2)
                .is_empty()
        );
    }

    #[test]
    fn the_joystick_tilts_past_its_deadzone_and_sprints_pushed_forward() {
        let (mut touch, layout, now) = playing(Movement::Joystick);
        let reach = f64::from(layout.reach());
        let landed = [300.0, 800.0];
        let at = |dx: f64, dy: f64| [landed[0] + dx * reach, landed[1] - dy * reach];
        touch.finger(&layout, 1, Phase::Start, landed, now);
        assert_eq!(touch.move_vector(), Some([0.0, 0.0]));
        touch.finger(&layout, 1, Phase::Move, at(0.1, 0.0), now);
        assert_eq!(touch.move_vector(), Some([0.0, 0.0]));
        // Pushed right: the player goes right, which the game counts as negative left.
        touch.finger(&layout, 1, Phase::Move, at(1.0, 0.0), now);
        let [left, forward] = touch.move_vector().unwrap();
        assert!(
            (left + 1.0).abs() < 1e-3 && forward.abs() < 1e-3,
            "{left} {forward}"
        );
        assert_eq!(
            touch.finger(&layout, 1, Phase::Move, at(0.0, 3.0), now),
            [Input::Sprint(true)]
        );
        assert_eq!(
            touch.finger(&layout, 1, Phase::End, at(0.0, 3.0), now),
            [Input::Sprint(false)]
        );
        assert_eq!(touch.move_vector(), None);
    }

    #[test]
    fn a_second_thumb_on_the_left_turns_the_view_and_leaves_the_joystick_alone() {
        let (mut touch, layout, now) = playing(Movement::Joystick);
        touch.finger(&layout, 1, Phase::Start, [300.0, 800.0], now);
        touch.finger(&layout, 2, Phase::Start, [200.0, 300.0], now);
        assert_eq!(
            touch.finger(&layout, 2, Phase::Move, [230.0, 300.0], now),
            [Input::Look([15.0, 0.0])]
        );
        assert!(touch.move_vector().is_some());
    }

    #[test]
    fn jump_holds_at_least_a_tick_and_a_quick_double_tap_presses_twice() {
        let (mut touch, layout, now) = playing(Movement::DPad);
        let jump = centre(layout.button(Button::Jump));
        let ms = |n: u64| now + Duration::from_millis(n);
        assert_eq!(
            touch.finger(&layout, 1, Phase::Start, jump, now),
            [Input::Mapping("key.jump", true)]
        );
        assert!(
            touch
                .finger(&layout, 1, Phase::End, jump, ms(40))
                .is_empty()
        );
        assert!(
            touch
                .finger(&layout, 2, Phase::Start, jump, ms(55))
                .is_empty()
        );
        let context = touch.context;
        assert_eq!(
            touch.frame(context, ms(70)),
            [Input::Mapping("key.jump", false)]
        );
        assert!(touch.frame(context, ms(100)).is_empty());
        assert_eq!(
            touch.frame(context, ms(140)),
            [Input::Mapping("key.jump", true)]
        );
        let pause = centre(layout.button(Button::Pause));
        assert_eq!(
            touch.finger(&layout, 3, Phase::Start, pause, ms(150)),
            [Input::Back]
        );
    }

    #[test]
    fn a_mapping_two_fingers_hold_is_released_by_the_last_one() {
        let (mut touch, layout, now) = playing(Movement::DPad);
        touch.finger(&layout, 3, Phase::Start, VIEW, now);
        touch.finger(&layout, 4, Phase::Start, [2200.0, 500.0], now);
        let context = touch.context;
        assert_eq!(
            touch.frame(context, now + HOLD),
            [Input::Mapping("key.attack", true)]
        );
        // A tap with a third finger neither taps nor lets go of the attack.
        touch.finger(&layout, 5, Phase::Start, [2400.0, 500.0], now + HOLD);
        assert!(
            touch
                .finger(&layout, 5, Phase::End, [2400.0, 500.0], now + HOLD)
                .is_empty()
        );
        assert!(
            touch
                .finger(&layout, 3, Phase::End, VIEW, now + HOLD)
                .is_empty()
        );
        assert_eq!(
            touch.finger(&layout, 4, Phase::End, VIEW, now + HOLD + MIN_STATE),
            [Input::Mapping("key.attack", false)]
        );
    }

    #[test]
    fn the_hotbar_selects_the_slot_under_the_finger() {
        let (mut touch, layout, now) = playing(Movement::DPad);
        // GUI 525 x 242 at scale 5: the hotbar spans x 171..353 above y 220.
        let slot = |i: f64| [(171.0 + 1.0 + 20.0 * i + 10.0) * 5.0, 230.0 * 5.0];
        assert_eq!(
            touch.finger(&layout, 1, Phase::Start, slot(0.0), now),
            [Input::Hotbar(0)]
        );
        assert_eq!(
            touch.finger(&layout, 1, Phase::Move, slot(8.0), now),
            [Input::Hotbar(8)]
        );
    }

    #[test]
    fn leaving_play_lets_go_of_everything_held() {
        let (mut touch, layout, now) = playing(Movement::DPad);
        let jump = centre(layout.button(Button::Jump));
        touch.finger(&layout, 1, Phase::Start, jump, now);
        touch.finger(&layout, 2, Phase::Start, VIEW, now);
        let context = touch.context;
        touch.frame(context, now + HOLD);
        touch.finger(&layout, 3, Phase::Start, cell(&layout, Cell::Forward), now);
        let menu = Context {
            mode: Mode::Menu,
            ..context
        };
        assert_eq!(
            touch.frame(menu, now + HOLD),
            [
                Input::Mapping("key.jump", false),
                Input::Mapping("key.attack", false)
            ]
        );
        assert_eq!(touch.move_vector(), None);
        assert!(touch.finger(&layout, 2, Phase::End, VIEW, now).is_empty());
    }
}
