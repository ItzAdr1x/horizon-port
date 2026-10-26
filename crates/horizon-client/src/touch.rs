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
//!   top corner pauses;
//! - key caps in the other top corner press F5 and F3, and Fn opens a panel of the other F-keys
//!   (`functions`).
//!
//! Nothing is drawn until a finger first touches the screen. This module is the state machine;
//! `layout` places the controls, `draw` draws them, and the game applies the `Input`s.

use std::time::{Duration, Instant};

mod draw;
mod functions;
mod keys;
mod layout;
mod menu;
pub use functions::{Function, Shown};
use keys::Keys;
pub use layout::{Button, Cell, Layout};
pub use menu::{Here, Loupe, MouseButton, PlaceOne, PlaceResult, SlotKind, SlotSpot, Spot};

/// A finger held still on the view this long mines, or uses the held item.
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

/// What the game tells the controls each frame, and with each touch.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Context {
    pub mode: Mode,
    /// What the screen shows, as an identity: the screen, its page, the menu behind it. When it
    /// changes, fingers on the old one do nothing more.
    pub scene: u64,
    pub movement: Movement,
    /// The player is flying: Jump goes up and the D-pad's middle down.
    pub flying: bool,
    /// The held item is used over time (food, a bow, a shield...): a held finger uses it.
    pub held_item_used_over_time: bool,
    /// An inventory is open: the Shift button is there.
    pub slots: bool,
    /// The HUD is drawn (F1 hides it): the hotbar's slots answer taps.
    pub hud_visible: bool,
    /// The hand holds a stack over an open inventory: taps place it, holds place one at a
    /// time, drags spread it.
    pub carrying: bool,
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
    /// The mouse wheel turns this many lines where the pointer is, up when positive.
    Wheel(f32),
    /// Shift goes down or up, as the physical key a keyboard's Shift is (`has_shift_down`).
    Shift(bool),
    /// If the hand holds a stack now, a left click at this point (physical pixels): a dragged
    /// stack put down, or what that leaves in hand put back where it came from. Never a result.
    PutDown([f64; 2]),
    /// One item from the hand into a slot, if the menu says the right click there moves exactly
    /// that, answered at once (`Touch::placed`).
    PlaceOne(PlaceOne),
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
    /// A key cap or a tile of the Fn panel tapped: its keys pressed and released whole.
    Function(Function),
}

/// What a finger does, decided where it lands.
#[derive(Clone, Debug)]
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
    /// On a key cap or a tile of the Fn panel: a tap if it lifts there.
    Function(&'static functions::Control),
    /// The D-pad's middle while flying: down.
    Descend,
    Hotbar,
    /// A finger that does nothing until it lifts: a second one on a menu or on the D-pad, the
    /// middle's tap, or one the scene changed under.
    Spent,
}

impl Role {
    /// The key mapping this finger holds down.
    fn holds(&self) -> Option<&'static str> {
        match *self {
            Role::Button(Button::Jump) => Some("key.jump"),
            Role::Button(Button::Inventory) => Some("key.inventory"),
            Role::Button(Button::Chat) => Some("key.chat"),
            Role::Descend => Some("key.sneak"),
            Role::Look { holding, .. } => holding,
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
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
    /// The Fn panel is open over play.
    panel: bool,
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

    /// A finger's event, on what the game shows now.
    pub fn touch(
        &mut self,
        layout: &Layout,
        context: Context,
        event: TouchEvent,
        now: Instant,
    ) -> Vec<Input> {
        let TouchEvent {
            id,
            phase,
            at,
            here,
        } = event;
        let mut out = Vec::new();
        self.follow(context, now, &mut out);
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
                let mut role = std::mem::replace(&mut self.fingers[i].role, Role::Spent);
                match &mut role {
                    Role::Menu(gesture) => {
                        self.slide_menu(layout, id, gesture, at, here, now, &mut out)
                    }
                    role => self.slide(layout, role, at, now, &mut out),
                }
                self.fingers[i].role = role;
            }
            (Phase::End, Some(i)) => {
                let finger = self.fingers.remove(i);
                match finger.role {
                    Role::Menu(gesture) => self.lift_menu(gesture, at, here, &mut out),
                    Role::Function(control) => {
                        self.lift_function(layout, control, at, now, &mut out)
                    }
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
        self.follow(context, now, &mut out);
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

    /// What the game shows now. Another mode or scene closes the Fn panel and ends what every
    /// finger was doing; the HUD hidden or shown ends a finger on the hotbar.
    fn follow(&mut self, context: Context, now: Instant, out: &mut Vec<Input>) {
        let changed = (context.mode, context.scene) != (self.context.mode, self.context.scene);
        if !context.slots {
            self.menus.shift.latch = menu::Latch::Off;
        }
        if context.hud_visible != self.context.hud_visible {
            for finger in &mut self.fingers {
                if matches!(finger.role, Role::Hotbar) {
                    finger.role = Role::Spent;
                }
            }
        }
        self.context = context;
        if !changed {
            return;
        }
        self.panel = false;
        self.spend_fingers(now, out);
    }

    /// Every finger does nothing more until it lifts: a button it held down comes up clicking
    /// nothing, and Shift lets go.
    fn spend_fingers(&mut self, now: Instant, out: &mut Vec<Input>) {
        for i in 0..self.fingers.len() {
            let role = std::mem::replace(&mut self.fingers[i].role, Role::Spent);
            match role {
                Role::Menu(gesture) => self.let_go(gesture, out),
                Role::Pad { .. } | Role::Stick { .. } => self.stop_moving(now, out),
                _ => {}
            }
        }
        self.menus.shift = Default::default();
        self.keys.release_all(now, out);
    }

    /// A finger holds `mapping` down.
    fn fingers_hold(&self, mapping: &str) -> bool {
        self.fingers.iter().any(|f| f.role.holds() == Some(mapping))
    }

    fn land(&mut self, layout: &Layout, at: [f64; 2], now: Instant, out: &mut Vec<Input>) -> Role {
        let px = [at[0] as f32, at[1] as f32];
        if let Some(role) = self.land_function(layout, px, now, out) {
            return role;
        }
        for button in [Button::Jump, Button::Inventory, Button::Pause, Button::Chat] {
            if layout.on(button, px) {
                let role = Role::Button(button);
                match role.holds() {
                    Some(mapping) => self.keys.set(mapping, true, now, out),
                    None => out.push(Input::Back),
                }
                return role;
            }
        }
        if self.context.hud_visible
            && let Some(slot) = layout.hotbar_slot(px)
        {
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
            Role::Menu(_) | Role::Button(_) | Role::Function(_) | Role::Descend | Role::Spent => {}
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
mod tests;
