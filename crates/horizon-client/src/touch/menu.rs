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
    /// The crafting grid's result: Shift crafts all it can.
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

/// What a finger that left where it landed at `from` presses for.
pub(super) fn drag(landed: &Here, from: [f64; 2], shifted: bool) -> Press {
    match landed.spot {
        _ if landed.carrying => Press::Spread,
        // Shifted, the press quick-moves the stack away: nothing is left in hand to put down.
        _ if shifted => Press::Plain,
        // A creative item's copy has nowhere to go back to.
        Spot::Slot(origin)
            if origin.filled && matches!(origin.kind, SlotKind::Normal | SlotKind::Result) =>
        {
            Press::Carry { origin, from }
        }
        _ => Press::Plain,
    }
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
    /// Whether a click is shifted, using Once up. `latched`: the latch may shift it; creative's
    /// trash slot, where Shift empties the whole inventory, is shifted only by a held button.
    fn take(&mut self, latched: bool) -> bool {
        if !latched {
            return self.held;
        }
        let active = self.active();
        if self.latch == Latch::Once {
            self.latch = Latch::Off;
        }
        active
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
            let shifted = self.shift_for(&landed);
            let press = drag(&landed, from, shifted);
            out.push(Input::Pointer(from));
            press_down(out, press.button(), shifted, true);
            *gesture = Gesture::Pressing {
                press,
                shifted,
                outside: None,
            };
        }
        match gesture {
            Gesture::Shift => return,
            Gesture::Pressing { press, outside, .. } => {
                *outside = (press.throws() && here.spot == Spot::Outside).then_some(at);
            }
            _ => {}
        }
        out.push(Input::Pointer(at));
    }

    /// Whether a click on where a finger landed is shifted, as the Shift button stands.
    fn shift_for(&mut self, landed: &Here) -> bool {
        let trash = matches!(landed.spot, Spot::Slot(s) if s.kind == SlotKind::Trash);
        self.menus.shift.take(!trash)
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
        out.push(Input::Pointer(at));
        match gesture {
            // Fingers miss on a big screen, and a stack thrown into the world can't be taken
            // back: throwing is a drag beyond the panel, never a tap.
            Gesture::Pending { here: landed, .. }
                if landed.carrying && landed.spot == Spot::Outside => {}
            // Two taps on one slot reach the game as two clicks at their own lifts, so its double
            // click (PICKUP_ALL) is the game's to decide, on its own clock.
            Gesture::Pending { here: landed, .. } => {
                let shifted = self.shift_for(&landed);
                press_down(out, MouseButton::Left, shifted, false);
                release(out, MouseButton::Left, shifted);
                if landed.spot == Spot::Field {
                    out.push(Input::Keyboard);
                }
            }
            Gesture::Pressing {
                press: Press::Carry { origin, from },
                ..
            } => {
                out.push(Input::Release(MouseButton::Left));
                // The drop, and whatever it leaves in hand put back where the stack came from
                // (a stack it was swapped with, a remainder). A crafted stack goes back nowhere:
                // a click on the result would craft again.
                let back = origin.kind == SlotKind::Normal;
                match here.spot {
                    Spot::Slot(b) if b.id == origin.id => {}
                    Spot::Outside => click(out, MouseButton::Left, true),
                    Spot::Slot(_) => {
                        click(out, MouseButton::Left, true);
                        if back {
                            out.push(Input::PutBack(from));
                        }
                    }
                    _ if back => out.push(Input::PutBack(from)),
                    _ => {}
                }
            }
            Gesture::Pressing { press, shifted, .. } => release(out, press.button(), shifted),
            Gesture::Done | Gesture::Shift => {}
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

    /// Let go of what a menu finger holds without acting: a cancelled touch, or a screen that
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

    /// The game's screen changed under the menu fingers while it took their clicks: they do
    /// nothing more until they lift, and their buttons' releases never reach the new screen.
    pub fn spend(&mut self) -> Vec<Input> {
        let mut out = vec![Input::Pointer(AWAY)];
        for finger in &mut self.fingers {
            match finger.role {
                Role::Menu(Gesture::Shift) => {}
                Role::Menu(gesture) => {
                    if let Gesture::Pressing { shifted: true, .. } = gesture {
                        out.push(Input::Shift(false));
                    }
                    finger.role = Role::Spent;
                }
                _ => {}
            }
        }
        out
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
mod tests {
    use super::super::{Context, Phase, TouchEvent};
    use super::*;
    use std::time::Instant;

    const LEFT: MouseButton = MouseButton::Left;
    const TRASH_ID: u32 = u32::MAX;

    fn menu() -> (Touch, Layout, Instant) {
        let mut touch = Touch::default();
        let now = Instant::now();
        let context = Context {
            mode: Mode::Menu,
            ..Context::default()
        };
        assert!(touch.frame(context, now).is_empty());
        (touch, Layout::new(2622, 1206, 3.0), now)
    }

    fn event(id: u64, phase: Phase, at: [f64; 2], here: Here) -> TouchEvent {
        TouchEvent {
            id,
            phase,
            at,
            here,
        }
    }

    fn slot_of(id: u32, filled: bool, kind: SlotKind) -> Here {
        Here {
            spot: Spot::Slot(SlotSpot { id, filled, kind }),
            ..Here::default()
        }
    }

    /// A stack, under an empty hand.
    fn slot(id: u32) -> Here {
        slot_of(id, true, SlotKind::Normal)
    }

    fn empty(id: u32) -> Here {
        slot_of(id, false, SlotKind::Normal)
    }

    fn carrying(here: Here) -> Here {
        Here {
            carrying: true,
            ..here
        }
    }

    fn spot(spot: Spot) -> Here {
        Here {
            spot,
            ..Here::default()
        }
    }

    const fn press(fresh: bool) -> Input {
        Input::Press {
            button: LEFT,
            fresh,
        }
    }

    /// The inputs of a finger that lands at `A` over `landed`, moves to `B` over `over` and
    /// lifts there.
    fn drag_and_lift(landed: Here, over: Here) -> (Vec<Input>, Vec<Input>) {
        let (mut touch, layout, now) = menu();
        touch.touch(&layout, event(1, Phase::Start, A, landed), now);
        let moved = touch.touch(&layout, event(1, Phase::Move, B, over), now);
        let lifted = touch.touch(&layout, event(1, Phase::End, B, over), now);
        (moved, lifted)
    }

    const A: [f64; 2] = [1000.0, 800.0];
    const B: [f64; 2] = [1300.0, 800.0];

    #[test]
    fn a_finger_hovers_where_it_lands_and_clicks_where_it_lifts() {
        let (mut touch, layout, now) = menu();
        let at = [1010.0, 805.0];
        let plain = Here::default();
        assert_eq!(
            touch.touch(&layout, event(1, Phase::Start, A, plain), now),
            [Input::Pointer(A)]
        );
        assert_eq!(
            touch.touch(&layout, event(1, Phase::Move, at, plain), now),
            [Input::Pointer(at)]
        );
        assert_eq!(
            touch.touch(&layout, event(1, Phase::End, at, plain), now),
            [
                Input::Pointer(at),
                Input::Press {
                    button: LEFT,
                    fresh: false
                },
                Input::Release(LEFT),
                Input::Pointer(AWAY)
            ]
        );
    }

    #[test]
    fn a_cancelled_tap_hovers_away_without_clicking() {
        let (mut touch, layout, now) = menu();
        touch.touch(&layout, event(1, Phase::Start, A, Here::default()), now);
        assert_eq!(
            touch.touch(&layout, event(1, Phase::Cancel, A, Here::default()), now),
            [Input::Pointer(AWAY)]
        );
    }

    #[test]
    fn a_drag_on_a_button_presses_where_it_landed_then_follows() {
        let (mut touch, layout, now) = menu();
        let plain = Here::default();
        touch.touch(&layout, event(1, Phase::Start, A, plain), now);
        assert_eq!(
            touch.touch(&layout, event(1, Phase::Move, B, plain), now),
            [
                Input::Pointer(A),
                Input::Press {
                    button: LEFT,
                    fresh: true
                },
                Input::Pointer(B)
            ]
        );
        assert_eq!(
            touch.touch(&layout, event(1, Phase::End, B, plain), now),
            [
                Input::Pointer(B),
                Input::Release(LEFT),
                Input::Pointer(AWAY)
            ]
        );
    }

    #[test]
    fn a_cancel_mid_drag_lets_go_with_nothing_hovered() {
        let (mut touch, layout, now) = menu();
        touch.touch(&layout, event(1, Phase::Start, A, Here::default()), now);
        touch.touch(&layout, event(1, Phase::Move, B, Here::default()), now);
        assert_eq!(
            touch.touch(&layout, event(1, Phase::Cancel, B, Here::default()), now),
            [Input::Pointer(AWAY), Input::Abort(LEFT)]
        );
    }

    #[test]
    fn leaving_a_menu_mid_drag_lets_go_too() {
        let (mut touch, layout, now) = menu();
        touch.touch(&layout, event(1, Phase::Start, A, Here::default()), now);
        touch.touch(&layout, event(1, Phase::Move, B, Here::default()), now);
        let playing = Context {
            mode: Mode::Playing,
            ..Context::default()
        };
        assert_eq!(
            touch.frame(playing, now),
            [Input::Pointer(AWAY), Input::Abort(LEFT)]
        );
        assert!(
            touch
                .touch(&layout, event(1, Phase::End, B, Here::default()), now)
                .is_empty()
        );
    }

    #[test]
    fn a_tap_that_wanders_inside_its_slot_is_still_a_tap() {
        let (mut touch, layout, now) = menu();
        touch.touch(&layout, event(1, Phase::Start, A, slot(3)), now);
        let far = [A[0] + 60.0, A[1]];
        assert_eq!(
            touch.touch(&layout, event(1, Phase::Move, far, slot(3)), now),
            [Input::Pointer(far)]
        );
        assert!(matches!(
            touch.touch(&layout, event(1, Phase::End, far, slot(3)), now)[..],
            [
                _,
                Input::Press { fresh: false, .. },
                Input::Release(LEFT),
                _
            ]
        ));
    }

    #[test]
    fn left_is_the_slot_for_slots_and_the_slop_elsewhere() {
        let slop = 32.0;
        let near = [A[0] + 20.0, A[1]];
        let far = [A[0] + 40.0, A[1]];
        let plain = Here::default();
        assert!(!left(&plain, A, &plain, near, slop));
        assert!(left(&plain, A, &plain, far, slop));
        assert!(!left(&slot(3), A, &slot(3), far, slop));
        assert!(left(&slot(3), A, &slot(4), near, slop));
        assert!(left(&slot(3), A, &plain, near, slop));
        // In a list, a slot is a row like any other: the slop decides.
        let listed = Here {
            list: Some(54.0),
            ..slot(3)
        };
        assert!(!left(&listed, A, &listed, near, slop));
        assert!(left(&listed, A, &listed, far, slop));
    }

    #[test]
    fn a_stack_dragged_with_an_empty_hand_drops_where_it_lifts() {
        let (moved, lifted) = drag_and_lift(slot(3), empty(4));
        assert_eq!(moved, [Input::Pointer(A), press(true), Input::Pointer(B)]);
        assert_eq!(
            lifted,
            [
                Input::Pointer(B),
                Input::Release(LEFT),
                press(true),
                Input::Release(LEFT),
                Input::PutBack(A),
                Input::Pointer(AWAY)
            ]
        );
    }

    #[test]
    fn a_stack_dropped_on_a_gap_goes_back_where_it_came_from() {
        let (_, lifted) = drag_and_lift(slot(3), Here::default());
        assert_eq!(
            lifted,
            [
                Input::Pointer(B),
                Input::Release(LEFT),
                Input::PutBack(A),
                Input::Pointer(AWAY)
            ]
        );
    }

    #[test]
    fn a_stack_dragged_back_onto_its_slot_stays_in_hand() {
        let (mut touch, layout, now) = menu();
        touch.touch(&layout, event(1, Phase::Start, A, slot(3)), now);
        touch.touch(&layout, event(1, Phase::Move, B, empty(4)), now);
        touch.touch(&layout, event(1, Phase::Move, A, slot(3)), now);
        assert_eq!(
            touch.touch(&layout, event(1, Phase::End, A, slot(3)), now),
            [
                Input::Pointer(A),
                Input::Release(LEFT),
                Input::Pointer(AWAY)
            ]
        );
    }

    #[test]
    fn a_stack_dragged_beyond_the_panel_is_thrown_and_hinted() {
        let (mut touch, layout, now) = menu();
        touch.touch(&layout, event(1, Phase::Start, A, slot(3)), now);
        touch.touch(&layout, event(1, Phase::Move, B, spot(Spot::Outside)), now);
        assert!(matches!(
            touch.fingers[0].role,
            Role::Menu(Gesture::Pressing {
                outside: Some(B),
                ..
            })
        ));
        assert_eq!(
            touch.touch(&layout, event(1, Phase::End, B, spot(Spot::Outside)), now),
            [
                Input::Pointer(B),
                Input::Release(LEFT),
                press(true),
                Input::Release(LEFT),
                Input::Pointer(AWAY)
            ]
        );
    }

    #[test]
    fn a_crafted_stack_dropped_on_another_is_never_put_back() {
        let result = slot_of(200, true, SlotKind::Result);
        let (_, lifted) = drag_and_lift(result, slot(4));
        assert!(!lifted.iter().any(|i| matches!(i, Input::PutBack(_))));
        assert_eq!(lifted.iter().filter(|i| **i == press(true)).count(), 1);
    }

    #[test]
    fn a_creative_copy_is_dragged_as_the_mouse_drags_it() {
        let copy = slot_of(901, true, SlotKind::Display);
        let (_, lifted) = drag_and_lift(copy, empty(4));
        assert_eq!(
            lifted,
            [
                Input::Pointer(B),
                Input::Release(LEFT),
                Input::Pointer(AWAY)
            ]
        );
    }

    #[test]
    fn a_carried_stack_dragged_across_slots_presses_on_the_first_as_it_leaves() {
        let (mut touch, layout, now) = menu();
        touch.touch(&layout, event(1, Phase::Start, A, carrying(empty(3))), now);
        let inside = [A[0] + 10.0, A[1]];
        assert_eq!(
            touch.touch(
                &layout,
                event(1, Phase::Move, inside, carrying(empty(3))),
                now
            ),
            [Input::Pointer(inside)]
        );
        assert_eq!(
            touch.touch(&layout, event(1, Phase::Move, B, carrying(empty(4))), now),
            [Input::Pointer(A), press(true), Input::Pointer(B)]
        );
        assert_eq!(
            touch.touch(&layout, event(1, Phase::End, B, carrying(empty(4))), now),
            [
                Input::Pointer(B),
                Input::Release(LEFT),
                Input::Pointer(AWAY)
            ]
        );
    }

    #[test]
    fn a_tap_outside_with_a_stack_in_hand_throws_nothing() {
        let (mut touch, layout, now) = menu();
        let outside = carrying(spot(Spot::Outside));
        touch.touch(&layout, event(1, Phase::Start, A, outside), now);
        assert_eq!(
            touch.touch(&layout, event(1, Phase::End, A, outside), now),
            [Input::Pointer(A), Input::Pointer(AWAY)]
        );
        // With nothing in hand there is nothing to throw, and the click is the mouse's.
        touch.touch(&layout, event(2, Phase::Start, A, spot(Spot::Outside)), now);
        assert!(
            touch
                .touch(&layout, event(2, Phase::End, A, spot(Spot::Outside)), now)
                .contains(&press(false))
        );
    }

    #[test]
    fn a_tap_on_a_field_asks_for_the_keyboard() {
        let (mut touch, layout, now) = menu();
        touch.touch(&layout, event(1, Phase::Start, A, spot(Spot::Field)), now);
        assert_eq!(
            touch.touch(&layout, event(1, Phase::End, A, spot(Spot::Field)), now),
            [
                Input::Pointer(A),
                press(false),
                Input::Release(LEFT),
                Input::Keyboard,
                Input::Pointer(AWAY)
            ]
        );
    }

    #[test]
    fn two_taps_on_a_slot_are_two_clicks_at_their_lifts() {
        let (mut touch, layout, now) = menu();
        let tap = [
            Input::Pointer(A),
            press(false),
            Input::Release(LEFT),
            Input::Pointer(AWAY),
        ];
        for id in [1, 2] {
            assert_eq!(
                touch.touch(&layout, event(id, Phase::Start, A, slot(3)), now),
                [Input::Pointer(A)]
            );
            assert_eq!(
                touch.touch(&layout, event(id, Phase::End, A, slot(3)), now),
                tap
            );
        }
    }

    #[test]
    fn a_drag_right_after_a_tap_on_the_same_slot_is_no_double_click() {
        let (mut touch, layout, now) = menu();
        touch.touch(&layout, event(1, Phase::Start, A, slot(3)), now);
        touch.touch(&layout, event(1, Phase::End, A, slot(3)), now);
        // The tap picked the stack up; the next finger spreads it from the same slot.
        touch.touch(&layout, event(2, Phase::Start, A, carrying(empty(3))), now);
        let moved = touch.touch(&layout, event(2, Phase::Move, B, carrying(empty(4))), now);
        assert!(moved.contains(&press(true)) && !moved.contains(&press(false)));
    }

    #[test]
    fn a_screen_change_spends_the_fingers() {
        let (mut touch, layout, now) = menu();
        touch.touch(&layout, event(1, Phase::Start, A, Here::default()), now);
        touch.touch(&layout, event(1, Phase::Move, B, Here::default()), now);
        assert_eq!(touch.spend(), [Input::Pointer(AWAY)]);
        assert!(
            touch
                .touch(&layout, event(1, Phase::Move, A, Here::default()), now)
                .is_empty()
        );
        assert!(
            touch
                .touch(&layout, event(1, Phase::End, A, Here::default()), now)
                .is_empty()
        );
        // The next finger is the pointer again.
        assert_eq!(
            touch.touch(&layout, event(2, Phase::Start, A, Here::default()), now),
            [Input::Pointer(A)]
        );
    }

    #[test]
    fn the_last_lift_is_where_a_carried_stack_rests() {
        let (mut touch, layout, now) = menu();
        assert_eq!(touch.rest(), None);
        touch.touch(&layout, event(1, Phase::Start, A, slot(3)), now);
        touch.touch(&layout, event(1, Phase::End, B, slot(3)), now);
        assert_eq!(touch.rest(), Some(B));
    }

    #[test]
    fn drag_decides_from_where_it_landed() {
        let origin = |here: Here| match here.spot {
            Spot::Slot(origin) => origin,
            _ => unreachable!(),
        };
        assert_eq!(
            drag(&slot(3), A, false),
            Press::Carry {
                origin: origin(slot(3)),
                from: A
            }
        );
        let result = slot_of(200, true, SlotKind::Result);
        assert!(matches!(drag(&result, A, false), Press::Carry { .. }));
        assert_eq!(drag(&empty(3), A, false), Press::Plain);
        assert_eq!(
            drag(&slot_of(901, true, SlotKind::Display), A, false),
            Press::Plain
        );
        assert_eq!(drag(&carrying(empty(3)), A, false), Press::Spread);
        assert_eq!(drag(&carrying(slot(3)), A, false), Press::Spread);
        assert_eq!(
            drag(&carrying(spot(Spot::Outside)), A, false),
            Press::Spread
        );
        assert_eq!(drag(&Here::default(), A, false), Press::Plain);
        // Shifted, a stack quick-moves at the press: there is nothing to carry.
        assert_eq!(drag(&slot(3), A, true), Press::Plain);
        assert_eq!(drag(&carrying(empty(3)), A, true), Press::Spread);
    }

    const RIGHT: MouseButton = MouseButton::Right;

    /// The inputs of a finger that lands at `A` over `landed` and is held still past `MENU_HOLD`.
    fn long_press(landed: Here) -> (Touch, Layout, Instant, Vec<Input>) {
        let (mut touch, layout, now) = menu();
        touch.touch(&layout, event(1, Phase::Start, A, landed), now);
        let context = touch.context;
        // Play's hold is not long enough: a finger resting to read a tooltip still hovers.
        assert!(touch.frame(context, now + super::super::HOLD).is_empty());
        let held = touch.frame(context, now + MENU_HOLD);
        (touch, layout, now + MENU_HOLD, held)
    }

    const fn right_down() -> Input {
        Input::Press {
            button: RIGHT,
            fresh: true,
        }
    }

    #[test]
    fn a_long_press_on_a_stack_takes_half_with_the_right_button() {
        let (mut touch, layout, now, held) = long_press(slot(3));
        assert_eq!(held, [Input::Pointer(A), right_down()]);
        assert_eq!(
            touch.touch(&layout, event(1, Phase::End, A, slot(3)), now),
            [
                Input::Pointer(A),
                Input::Release(RIGHT),
                Input::Pointer(AWAY)
            ]
        );
    }

    #[test]
    fn a_long_press_with_a_stack_in_hand_places_one() {
        let (mut touch, layout, now, held) = long_press(carrying(empty(3)));
        assert_eq!(held, [Input::Pointer(A), right_down()]);
        assert_eq!(
            touch.touch(&layout, event(1, Phase::End, A, carrying(empty(3))), now),
            [
                Input::Pointer(A),
                Input::Release(RIGHT),
                Input::Pointer(AWAY)
            ]
        );
    }

    #[test]
    fn a_long_press_then_a_drag_places_one_per_slot() {
        let (mut touch, layout, now, _) = long_press(carrying(empty(3)));
        assert_eq!(
            touch.touch(&layout, event(1, Phase::Move, B, carrying(empty(4))), now),
            [Input::Pointer(B)]
        );
        assert_eq!(
            touch.touch(&layout, event(1, Phase::End, B, carrying(empty(4))), now),
            [
                Input::Pointer(B),
                Input::Release(RIGHT),
                Input::Pointer(AWAY)
            ]
        );
    }

    #[test]
    fn a_long_press_beyond_the_panel_drops_one_and_shows_it() {
        let outside = carrying(spot(Spot::Outside));
        let (touch, ..) = long_press(outside);
        assert!(matches!(
            touch.fingers[0].role,
            Role::Menu(Gesture::Pressing {
                press: Press::One,
                outside: Some(A),
                ..
            })
        ));
    }

    #[test]
    fn a_long_press_on_the_result_takes_one_craft_at_once() {
        let result = slot_of(200, true, SlotKind::Result);
        let (mut touch, layout, now, held) = long_press(result);
        assert_eq!(
            held,
            [Input::Pointer(A), right_down(), Input::Release(RIGHT)]
        );
        assert_eq!(
            touch.touch(&layout, event(1, Phase::End, A, result), now),
            [Input::Pointer(A), Input::Pointer(AWAY)]
        );
    }

    #[test]
    fn a_long_press_on_a_creative_copy_takes_a_full_stack() {
        let (_, _, _, held) = long_press(slot_of(901, true, SlotKind::Display));
        let middle = MouseButton::Middle;
        assert_eq!(
            held,
            [
                Input::Pointer(A),
                Input::Press {
                    button: middle,
                    fresh: true
                },
                Input::Release(middle)
            ]
        );
    }

    #[test]
    fn a_long_press_on_a_recipe_opens_its_alternatives_once() {
        let (_, _, _, held) = long_press(spot(Spot::Secondary));
        assert_eq!(
            held,
            [Input::Pointer(A), right_down(), Input::Release(RIGHT)]
        );
    }

    #[test]
    fn a_long_press_on_a_plain_button_keeps_hovering_and_still_taps() {
        let (mut touch, layout, now, held) = long_press(Here::default());
        assert!(held.is_empty());
        assert!(matches!(
            touch.fingers[0].role,
            Role::Menu(Gesture::Pending { held: true, .. })
        ));
        assert_eq!(
            touch.touch(&layout, event(1, Phase::End, A, Here::default()), now),
            [
                Input::Pointer(A),
                press(false),
                Input::Release(LEFT),
                Input::Pointer(AWAY)
            ]
        );
    }

    #[test]
    fn a_cancel_during_one_per_slot_lets_the_right_button_go() {
        let (mut touch, layout, now, _) = long_press(carrying(empty(3)));
        touch.touch(&layout, event(1, Phase::Move, B, carrying(empty(4))), now);
        assert_eq!(
            touch.touch(&layout, event(1, Phase::Cancel, B, carrying(empty(4))), now),
            [Input::Pointer(AWAY), Input::Abort(RIGHT)]
        );
    }

    #[test]
    fn hold_decides_from_where_it_landed() {
        assert_eq!(hold(&slot(3)), Hold::Press(Press::Split));
        assert_eq!(hold(&empty(3)), Hold::Nothing);
        assert_eq!(hold(&carrying(empty(3))), Hold::Press(Press::One));
        assert_eq!(hold(&carrying(slot(3))), Hold::Press(Press::One));
        assert_eq!(
            hold(&carrying(spot(Spot::Outside))),
            Hold::Press(Press::One)
        );
        assert_eq!(hold(&spot(Spot::Outside)), Hold::Nothing);
        let result = slot_of(200, true, SlotKind::Result);
        assert_eq!(hold(&result), Hold::Click(RIGHT));
        assert_eq!(hold(&slot_of(200, false, SlotKind::Result)), Hold::Nothing);
        let copy = slot_of(901, true, SlotKind::Display);
        assert_eq!(hold(&copy), Hold::Click(MouseButton::Middle));
        let trash = slot_of(7, false, SlotKind::Trash);
        assert_eq!(hold(&trash), Hold::Nothing);
        assert_eq!(hold(&carrying(trash)), Hold::Nothing);
        assert_eq!(hold(&spot(Spot::Secondary)), Hold::Click(RIGHT));
        for plain in [Spot::Plain, Spot::Slider, Spot::Grab, Spot::Field] {
            assert_eq!(hold(&spot(plain)), Hold::Nothing);
        }
    }

    #[test]
    fn tooltips_clear_a_finger_only_while_it_rests() {
        let (mut touch, layout, now) = menu();
        assert_eq!(touch.tooltip_lift(&layout), 0.0);
        touch.touch(&layout, event(1, Phase::Start, A, slot(3)), now);
        assert_eq!(touch.tooltip_lift(&layout), layout.finger_clearance());
        touch.touch(&layout, event(1, Phase::End, A, slot(3)), now);
        assert_eq!(touch.tooltip_lift(&layout), 0.0);
    }

    fn inventory() -> (Touch, Layout, Instant) {
        let (mut touch, layout, now) = menu();
        let context = Context {
            slots: true,
            ..touch.context
        };
        touch.frame(context, now);
        (touch, layout, now)
    }

    fn shift_button(layout: &Layout) -> [f64; 2] {
        let [x, y, w, h] = layout.button(Button::Shift);
        [f64::from(x + w / 2.0), f64::from(y + h / 2.0)]
    }

    /// A tap of finger `id` at `A` over `here`, and whether its click was shifted.
    fn tap_shifted(touch: &mut Touch, layout: &Layout, id: u64, here: Here, now: Instant) -> bool {
        touch.touch(layout, event(id, Phase::Start, A, here), now);
        let out = touch.touch(layout, event(id, Phase::End, A, here), now);
        let shifted = out.contains(&Input::Shift(true));
        assert_eq!(shifted, out.contains(&Input::Shift(false)), "{out:?}");
        if shifted {
            assert_eq!(
                out,
                [
                    Input::Pointer(A),
                    Input::Shift(true),
                    press(false),
                    Input::Release(LEFT),
                    Input::Shift(false),
                    Input::Pointer(AWAY)
                ]
            );
        }
        shifted
    }

    fn tap_shift(touch: &mut Touch, layout: &Layout, now: Instant) {
        let at = shift_button(layout);
        assert!(
            touch
                .touch(layout, event(9, Phase::Start, at, Here::default()), now)
                .is_empty()
        );
        assert!(
            touch
                .touch(layout, event(9, Phase::End, at, Here::default()), now)
                .is_empty()
        );
    }

    #[test]
    fn shift_tapped_shifts_the_next_click_only() {
        let (mut touch, layout, now) = inventory();
        tap_shift(&mut touch, &layout, now);
        assert!(tap_shifted(&mut touch, &layout, 1, slot(3), now));
        assert!(!tap_shifted(&mut touch, &layout, 2, slot(3), now));
    }

    #[test]
    fn shift_tapped_twice_quickly_locks_until_tapped_again() {
        let (mut touch, layout, now) = inventory();
        tap_shift(&mut touch, &layout, now);
        tap_shift(&mut touch, &layout, now + DOUBLE_TAP / 2);
        for id in 1..4 {
            assert!(tap_shifted(&mut touch, &layout, id, slot(3), now));
        }
        tap_shift(&mut touch, &layout, now + DOUBLE_TAP * 4);
        assert!(!tap_shifted(&mut touch, &layout, 5, slot(3), now));
    }

    #[test]
    fn shift_held_shifts_every_click_until_it_lifts() {
        let (mut touch, layout, now) = inventory();
        let at = shift_button(&layout);
        touch.touch(&layout, event(9, Phase::Start, at, Here::default()), now);
        // The finger on Shift leaves the pointer to another one.
        for id in 1..4 {
            assert!(tap_shifted(&mut touch, &layout, id, slot(3), now));
        }
        touch.touch(&layout, event(9, Phase::End, at, Here::default()), now);
        assert!(!tap_shifted(&mut touch, &layout, 4, slot(3), now));
    }

    #[test]
    fn the_shift_latch_never_reaches_the_trash() {
        let (mut touch, layout, now) = inventory();
        let trash = slot_of(TRASH_ID, false, SlotKind::Trash);
        tap_shift(&mut touch, &layout, now);
        assert!(!tap_shifted(&mut touch, &layout, 1, trash, now));
        // The latch is still set for the next slot.
        assert!(tap_shifted(&mut touch, &layout, 2, slot(3), now));
        // A finger holding Shift down shifts the trash: that is deliberate.
        let at = shift_button(&layout);
        touch.touch(
            &layout,
            event(9, Phase::Start, at, Here::default()),
            now + DOUBLE_TAP * 2,
        );
        assert!(tap_shifted(&mut touch, &layout, 3, trash, now));
    }

    #[test]
    fn a_shifted_drag_from_a_stack_quick_moves_it_and_puts_nothing_down() {
        let (mut touch, layout, now) = inventory();
        tap_shift(&mut touch, &layout, now);
        touch.touch(&layout, event(1, Phase::Start, A, slot(3)), now);
        assert_eq!(
            touch.touch(&layout, event(1, Phase::Move, B, empty(4)), now),
            [
                Input::Pointer(A),
                Input::Shift(true),
                press(true),
                Input::Pointer(B)
            ]
        );
        assert_eq!(
            touch.touch(&layout, event(1, Phase::End, B, empty(4)), now),
            [
                Input::Pointer(B),
                Input::Release(LEFT),
                Input::Shift(false),
                Input::Pointer(AWAY)
            ]
        );
    }

    #[test]
    fn a_cancelled_shifted_drag_lets_shift_go_too() {
        let (mut touch, layout, now) = inventory();
        tap_shift(&mut touch, &layout, now);
        touch.touch(&layout, event(1, Phase::Start, A, carrying(empty(3))), now);
        touch.touch(&layout, event(1, Phase::Move, B, carrying(empty(4))), now);
        assert_eq!(
            touch.touch(&layout, event(1, Phase::Cancel, B, carrying(empty(4))), now),
            [
                Input::Pointer(AWAY),
                Input::Abort(LEFT),
                Input::Shift(false)
            ]
        );
    }

    #[test]
    fn shift_is_only_in_inventories_and_unlocks_when_one_closes() {
        let (mut touch, layout, now) = menu();
        let at = shift_button(&layout);
        assert_eq!(
            touch.touch(&layout, event(1, Phase::Start, at, Here::default()), now),
            [Input::Pointer(at)]
        );
        touch.touch(&layout, event(1, Phase::Cancel, at, Here::default()), now);
        let (mut touch, layout, now) = inventory();
        tap_shift(&mut touch, &layout, now);
        tap_shift(&mut touch, &layout, now);
        let closed = Context {
            slots: false,
            ..touch.context
        };
        touch.frame(closed, now);
        touch.frame(
            Context {
                slots: true,
                ..closed
            },
            now,
        );
        assert!(!tap_shifted(&mut touch, &layout, 1, slot(3), now));
    }

    #[test]
    fn the_shift_key_cycles_off_once_locked() {
        let now = Instant::now();
        let mut key = ShiftKey::default();
        let tap = |key: &mut ShiftKey, at| {
            key.tap(at);
            key.lift();
            key.latch
        };
        assert_eq!(tap(&mut key, now), Latch::Once);
        assert_eq!(tap(&mut key, now + DOUBLE_TAP), Latch::Locked);
        assert_eq!(tap(&mut key, now + DOUBLE_TAP * 5), Latch::Off);
        assert_eq!(tap(&mut key, now + DOUBLE_TAP * 9), Latch::Once);
        // Too slow a second tap lets go instead of locking.
        assert_eq!(tap(&mut key, now + DOUBLE_TAP * 11), Latch::Off);
        assert!(!key.take(true));
        // Held, it shifts every click; the Once its tap set is used up by the first.
        key.tap(now + DOUBLE_TAP * 20);
        assert!(key.take(true) && key.take(true));
        key.lift();
        assert!(!key.take(true));
    }
}
