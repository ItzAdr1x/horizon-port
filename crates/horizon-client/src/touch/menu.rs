//! Fingers on the title screen and a world's menus. A finger is the mouse, but its button waits
//! until the gesture is known: a tap clicks where it lifts, and a drag presses where it landed.
//! The game says what lies under each finger (`Here`); this module answers with what a mouse
//! would send (`Input`).

use super::layout::{Button, Layout};
use super::{AWAY, DOUBLE_TAP, Finger, Input, Mode, Role, Touch};
use std::time::{Duration, Instant};

/// A menu finger held still this long is the right button. Longer than play's hold, which
/// splits a stack under a finger resting to read its tooltip: half a second, iOS's own long
/// press.
pub(super) const MENU_HOLD: Duration = Duration::from_millis(500);

/// What the game shows under a finger, told afresh with every touch event in a menu. The
/// landing one decides the gesture; later ones say where a drag is.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Here {
    pub spot: Spot,
    /// Inside a list that scrolls: the physical pixels one wheel line moves it.
    pub list: Option<f32>,
    /// The hand holds a stack: taps place it, holds place one, drags spread it.
    pub carrying: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Spot {
    /// Buttons, rows, labels, the background: a tap clicks, a drag is the mouse's drag.
    #[default]
    Plain,
    Slot(SlotSpot),
    /// Beyond the container's panel (slot -999): a click there throws what is carried.
    Outside,
    /// A slider's track: inside a list only a sideways drag moves it.
    Slider,
    /// A scrollbar: a drag holds it, even inside its list.
    Grab,
    /// A text field: a tap focuses it and asks for the keyboard.
    Field,
    /// A button whose right click means something (a recipe with alternatives).
    Secondary,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlotSpot {
    /// The game's slot id, opaque here: the same slot always answers the same id.
    pub id: u32,
    pub filled: bool,
    pub kind: SlotKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotKind {
    Normal,
    /// A result or output (the crafting grid's, a station's, a furnace's): a take crafts, trades
    /// or smelts, nothing is put there, and Shift on the crafting grid's crafts all it can.
    Result,
    /// A creative item tab's slot: clicks make copies.
    Display,
    /// Creative's destroy slot: with Shift it empties the whole inventory.
    Trash,
}

/// The lib's own mouse buttons; the bin maps them to winit's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    /// The pick block button: a full copy in creative (CLONE).
    Middle,
}

/// What a finger on a menu is doing, from landing to lifting.
#[derive(Clone, Copy, Debug)]
pub(super) enum Gesture {
    /// Down and hovering where it landed; nothing is pressed until the gesture is known.
    Pending {
        landed: Instant,
        from: [f64; 2],
        here: Here,
        /// Held past `MENU_HOLD` on something a hold doesn't change: still a tap.
        held: bool,
    },
    /// A mouse button is down: pressed where the finger landed, the pointer following it.
    Pressing {
        press: Press,
        /// Shift went down with the press and goes up with its release.
        shifted: bool,
        /// Beyond the panel at this point, where lifting throws what is carried: the hint.
        outside: Option<[f64; 2]>,
    },
    /// A list scrolls under the finger: wheel lines where it landed.
    Scrolling {
        from: [f64; 2],
        last: f64,
        /// Pixels dragged that don't make a whole line yet.
        rest: f64,
        line: f32,
    },
    /// A long press clicked already: lifting only hovers away.
    Done,
    /// On the Shift button.
    Shift,
}

impl Gesture {
    /// The finger is the mouse pointer: anything but the Shift button.
    fn points(self) -> bool {
        !matches!(self, Gesture::Shift)
    }
}

/// What a menu finger's button is down for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Press {
    /// The mouse's own drag: a slider, a scrollbar, a button pressed and slid off.
    Plain,
    /// A stack picked up with an empty hand from `origin`, where the finger landed at `from`:
    /// it is put down where the finger lifts.
    Carry { origin: SlotSpot, from: [f64; 2] },
    /// The carried stack, spread evenly over the slots the finger crosses (QUICK_CRAFT 0).
    Spread,
    /// The right button on a stack, under an empty hand: half of it taken (PICKUP 1).
    Split,
    /// The right button with a stack in hand: one placed on each slot the finger crosses, or
    /// dropped beyond the panel (QUICK_CRAFT 1).
    One,
}

impl Press {
    fn button(self) -> MouseButton {
        match self {
            Press::Split | Press::One => MouseButton::Right,
            Press::Plain | Press::Carry { .. } | Press::Spread => MouseButton::Left,
        }
    }
    /// Lifting it beyond the panel throws a stack.
    fn throws(self) -> bool {
        !matches!(self, Press::Plain | Press::Split)
    }
}

/// What holding a pending finger still for `MENU_HOLD` does: the mouse's right button, one rule
/// everywhere.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Hold {
    /// The button goes down, and comes up where the finger lifts.
    Press(Press),
    /// A click at once.
    Click(MouseButton),
    /// Nothing: the finger keeps hovering, and still taps when it lifts.
    Nothing,
}

/// What a hold on where the finger landed does.
pub(super) fn hold(landed: &Here) -> Hold {
    match landed.spot {
        Spot::Slot(s) if landed.carrying && s.kind != SlotKind::Trash => Hold::Press(Press::One),
        Spot::Outside if landed.carrying => Hold::Press(Press::One),
        Spot::Slot(s) if s.filled && !landed.carrying => match s.kind {
            SlotKind::Normal => Hold::Press(Press::Split),
            // Vanilla's right click on the result takes one craft, as a left one does.
            SlotKind::Result => Hold::Click(MouseButton::Right),
            // The pick block button: a full stack of the copy.
            SlotKind::Display => Hold::Click(MouseButton::Middle),
            SlotKind::Trash => Hold::Nothing,
        },
        Spot::Secondary => Hold::Click(MouseButton::Right),
        _ => Hold::Nothing,
    }
}

/// The finger has left where it landed: its slot, for a slot outside a list (a tap may wander
/// inside a slot); its slop anywhere else.
pub(super) fn left(landed: &Here, from: [f64; 2], here: &Here, at: [f64; 2], slop: f32) -> bool {
    match (landed.spot, landed.list) {
        (Spot::Slot(s), None) => !matches!(here.spot, Spot::Slot(t) if t.id == s.id),
        _ => ((at[0] - from[0]) as f32).hypot((at[1] - from[1]) as f32) > slop,
    }
}

/// What a finger becomes once it leaves where it landed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Drag {
    /// The mouse button goes down where it landed, for this.
    Press(Press),
    /// The list's wheel turns, a line every this many physical pixels.
    Scroll(f32),
}

/// What a finger that landed at `from` and is now at `at` becomes. In a list it scrolls, but a
/// scrollbar is held, and a slider or field moved sideways is the mouse's drag.
pub(super) fn drag(landed: &Here, from: [f64; 2], at: [f64; 2], shifted: bool) -> Drag {
    let sideways = (at[0] - from[0]).abs() > (at[1] - from[1]).abs();
    Drag::Press(match (landed.list, landed.spot) {
        (_, Spot::Grab) => Press::Plain,
        (Some(_), Spot::Slider | Spot::Field) if sideways => Press::Plain,
        (Some(line), _) => return Drag::Scroll(line),
        _ if landed.carrying => Press::Spread,
        // Shifted, the press quick-moves the stack away: nothing is left in hand to put down.
        _ if shifted => Press::Plain,
        // A creative item's copy has nowhere to go back to.
        (None, Spot::Slot(origin))
            if origin.filled && matches!(origin.kind, SlotKind::Normal | SlotKind::Result) =>
        {
            Press::Carry { origin, from }
        }
        _ => Press::Plain,
    })
}

/// Whole wheel lines in `dy` more pixels dragged, keeping what is left over: up (positive), the
/// list's top coming into view, when the finger goes down.
pub(super) fn wheel_lines(rest: &mut f64, dy: f64, line: f32) -> i32 {
    *rest += dy;
    let lines = (*rest / f64::from(line)).trunc();
    *rest -= lines * f64::from(line);
    lines as i32
}

/// A press and release of `button` where the pointer is.
fn click(out: &mut Vec<Input>, button: MouseButton, fresh: bool) {
    out.push(Input::Press { button, fresh });
    out.push(Input::Release(button));
}

/// The Shift button as the iPhone keyboard's shift key: a tap shifts the next click, a quick
/// second tap locks it, a tap unlocks, and holding it shifts every click until it lifts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Latch {
    #[default]
    Off,
    Once,
    Locked,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct ShiftKey {
    pub(super) latch: Latch,
    tapped: Option<Instant>,
    held: bool,
}

impl ShiftKey {
    /// A finger lands on the button: Off, then Once, then (quickly) Locked; a slow second tap,
    /// or a tap while locked, lets go.
    fn tap(&mut self, now: Instant) {
        let quick = self
            .tapped
            .is_some_and(|t| now.duration_since(t) <= DOUBLE_TAP);
        self.latch = match self.latch {
            Latch::Off => Latch::Once,
            Latch::Once if quick => Latch::Locked,
            Latch::Once | Latch::Locked => Latch::Off,
        };
        self.tapped = Some(now);
        self.held = true;
    }
    fn lift(&mut self) {
        self.held = false;
    }
    pub(super) fn active(&self) -> bool {
        self.held || self.latch != Latch::Off
    }
    /// Whether a click would be shifted. `latched`: the latch may shift it; creative's trash
    /// slot, where Shift empties the whole inventory, is shifted only by a held button.
    fn shifts(&self, latched: bool) -> bool {
        if latched { self.active() } else { self.held }
    }
    /// Whether a click is shifted, using Once up.
    fn take(&mut self, latched: bool) -> bool {
        let shifted = self.shifts(latched);
        if latched && self.latch == Latch::Once {
            self.latch = Latch::Off;
        }
        shifted
    }
}

impl Touch {
    /// A finger lands on the title screen or a menu.
    pub(super) fn land_menu(
        &mut self,
        layout: &Layout,
        at: [f64; 2],
        here: Here,
        now: Instant,
        out: &mut Vec<Input>,
    ) -> Role {
        let px = [at[0] as f32, at[1] as f32];
        if self.context.mode == Mode::Menu && layout.on(Button::Back, px) {
            out.push(Input::Back);
            return Role::Spent;
        }
        if self.context.slots && layout.on(Button::Shift, px) {
            self.menus.shift.tap(now);
            return Role::Menu(Gesture::Shift);
        }
        // A desk has one mouse.
        let pointing = |f: &Finger| matches!(f.role, Role::Menu(g) if g.points());
        if self.fingers.iter().any(pointing) {
            return Role::Spent;
        }
        out.push(Input::Pointer(at));
        Role::Menu(Gesture::Pending {
            landed: now,
            from: at,
            here,
            held: false,
        })
    }

    /// A menu finger moves to `at`, over `here`.
    pub(super) fn slide_menu(
        &mut self,
        layout: &Layout,
        gesture: &mut Gesture,
        at: [f64; 2],
        here: Here,
        out: &mut Vec<Input>,
    ) {
        if let Gesture::Pending {
            from, here: landed, ..
        } = *gesture
        {
            if !left(&landed, from, &here, at, layout.slop()) {
                out.push(Input::Pointer(at));
                return;
            }
            let latched = !trash(&landed);
            match drag(&landed, from, at, self.menus.shift.shifts(latched)) {
                Drag::Press(press) => {
                    let shifted = self.menus.shift.take(latched);
                    out.push(Input::Pointer(from));
                    press_down(out, press.button(), shifted, true);
                    *gesture = Gesture::Pressing {
                        press,
                        shifted,
                        outside: None,
                    };
                }
                Drag::Scroll(line) => {
                    // Nothing hovers while a list moves.
                    out.push(Input::Pointer(AWAY));
                    *gesture = Gesture::Scrolling {
                        from,
                        last: from[1],
                        rest: 0.0,
                        line,
                    };
                }
            }
        }
        match gesture {
            Gesture::Shift => return,
            Gesture::Scrolling {
                from,
                last,
                rest,
                line,
            } => {
                let lines = wheel_lines(rest, at[1] - *last, *line);
                *last = at[1];
                if lines != 0 {
                    // The list under where the finger landed turns, one line at a time, as a
                    // stepped list (a merchant's offers, creative's rows) takes them.
                    out.push(Input::Pointer(*from));
                    let step = Input::Wheel(lines.signum() as f32);
                    out.extend(std::iter::repeat_n(step, lines.unsigned_abs() as usize));
                    out.push(Input::Pointer(AWAY));
                }
                return;
            }
            Gesture::Pressing { press, outside, .. } => {
                *outside = (press.throws() && here.spot == Spot::Outside).then_some(at);
            }
            _ => {}
        }
        out.push(Input::Pointer(at));
    }

    /// Whether a click on `here` is shifted, as the Shift button stands.
    fn shift_for(&mut self, here: &Here) -> bool {
        self.menus.shift.take(!trash(here))
    }

    /// A menu finger lifts at `at`, over `here`.
    pub(super) fn lift_menu(
        &mut self,
        gesture: Gesture,
        at: [f64; 2],
        here: Here,
        out: &mut Vec<Input>,
    ) {
        if let Gesture::Shift = gesture {
            self.menus.shift.lift();
            return;
        }
        self.menus.rest = Some(at);
        if let Gesture::Scrolling { .. } = gesture {
            out.push(Input::Pointer(AWAY));
            return;
        }
        out.push(Input::Pointer(at));
        match gesture {
            // Fingers miss on a big screen, and a stack thrown into the world can't be taken
            // back: throwing is a drag beyond the panel, never a tap.
            Gesture::Pending { .. } if here.carrying && here.spot == Spot::Outside => {}
            // A tap clicks where it lifts. Two taps on one slot reach the game as two clicks at
            // their own lifts, so its double click (PICKUP_ALL) is the game's to decide, on its
            // own clock.
            Gesture::Pending { .. } => {
                let shifted = self.shift_for(&here);
                press_down(out, MouseButton::Left, shifted, false);
                release(out, MouseButton::Left, shifted);
                if here.spot == Spot::Field {
                    out.push(Input::Keyboard);
                }
            }
            Gesture::Pressing {
                press: Press::Carry { origin, from },
                ..
            } => {
                out.push(Input::Release(MouseButton::Left));
                // The stack put down where the finger lifts, and whatever that leaves in hand put
                // back where it came from (a stack it was swapped with, a remainder). A crafted
                // stack goes back nowhere: a click on the result would craft again.
                let back = origin.kind == SlotKind::Normal;
                match here.spot {
                    Spot::Slot(b) if b.id == origin.id => {}
                    Spot::Outside => click(out, MouseButton::Left, true),
                    Spot::Slot(_) => {
                        out.push(Input::PutDown(at));
                        if back {
                            out.push(Input::PutDown(from));
                        }
                    }
                    _ if back => out.push(Input::PutDown(from)),
                    _ => {}
                }
            }
            Gesture::Pressing { press, shifted, .. } => release(out, press.button(), shifted),
            Gesture::Scrolling { .. } | Gesture::Done | Gesture::Shift => {}
        }
        out.push(Input::Pointer(AWAY));
    }

    /// Once a frame: the menu finger held still long enough does what a hold does.
    pub(super) fn menu_holds(&mut self, now: Instant, out: &mut Vec<Input>) {
        for finger in &mut self.fingers {
            let Role::Menu(gesture) = &mut finger.role else {
                continue;
            };
            let Gesture::Pending {
                landed,
                from,
                here,
                held: held @ false,
            } = gesture
            else {
                continue;
            };
            if now.duration_since(*landed) < MENU_HOLD {
                continue;
            }
            let (from, here) = (*from, *here);
            match hold(&here) {
                Hold::Press(press) => {
                    let shifted = self.menus.shift.take(true);
                    out.push(Input::Pointer(from));
                    press_down(out, press.button(), shifted, true);
                    let outside = press.throws() && here.spot == Spot::Outside;
                    *gesture = Gesture::Pressing {
                        press,
                        shifted,
                        outside: outside.then_some(from),
                    };
                }
                Hold::Click(button) => {
                    out.push(Input::Pointer(from));
                    click(out, button, true);
                    *gesture = Gesture::Done;
                }
                Hold::Nothing => *held = true,
            }
        }
    }

    /// Let go of what a menu finger holds without acting: a cancelled touch, or a scene that
    /// changed under it. Nothing hovers when the button comes up, and its release clicks nothing.
    pub(super) fn let_go(&mut self, gesture: Gesture, out: &mut Vec<Input>) {
        if let Gesture::Shift = gesture {
            self.menus.shift.lift();
            return;
        }
        out.push(Input::Pointer(AWAY));
        if let Gesture::Pressing { press, shifted, .. } = gesture {
            out.push(Input::Abort(press.button()));
            if shifted {
                out.push(Input::Shift(false));
            }
        }
    }

    /// The physical pixels tooltips keep clear above the pointer: a fingertip while a finger is
    /// on a menu, none for a mouse.
    pub fn tooltip_lift(&self, layout: &Layout) -> f32 {
        let pointing = self
            .fingers
            .iter()
            .any(|f| matches!(f.role, Role::Menu(g) if g.points()));
        if pointing {
            layout.finger_clearance()
        } else {
            0.0
        }
    }

    /// Where a carried stack is drawn while nothing hovers: where the last finger lifted.
    pub fn rest(&self) -> Option<[f64; 2]> {
        self.menus.rest
    }
}

/// Creative's trash slot, which the Shift latch never reaches.
fn trash(here: &Here) -> bool {
    matches!(here.spot, Spot::Slot(s) if s.kind == SlotKind::Trash)
}

/// `button` goes down, Shift first when the press is shifted.
fn press_down(out: &mut Vec<Input>, button: MouseButton, shifted: bool, fresh: bool) {
    if shifted {
        out.push(Input::Shift(true));
    }
    out.push(Input::Press { button, fresh });
}

/// `button` comes up, then Shift if it went down with it.
fn release(out: &mut Vec<Input>, button: MouseButton, shifted: bool) {
    out.push(Input::Release(button));
    if shifted {
        out.push(Input::Shift(false));
    }
}

/// What menus keep between fingers.
#[derive(Debug, Default)]
pub(super) struct Menus {
    pub(super) shift: ShiftKey,
    /// Where the last menu finger lifted.
    rest: Option<[f64; 2]>,
}

#[cfg(test)]
mod tests;
