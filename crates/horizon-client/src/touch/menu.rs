//! Fingers on the title screen and a world's menus. A finger is the mouse, but its button waits
//! until the gesture is known: a tap clicks where it lifts, and a drag presses where it landed.
//! The game says what lies under each finger (`Here`); this module answers with what a mouse
//! would send (`Input`).

use super::layout::{Button, Layout};
use super::{AWAY, Input, Mode, Role, Touch};

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
    Pending { from: [f64; 2], here: Here },
    /// A mouse button is down: pressed where the finger landed, the pointer following it.
    Pressing {
        press: Press,
        /// Beyond the panel at this point, where lifting throws what is carried: the hint.
        outside: Option<[f64; 2]>,
    },
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
}

impl Press {
    fn button(self) -> MouseButton {
        MouseButton::Left
    }
    /// Lifting it beyond the panel throws a stack.
    fn throws(self) -> bool {
        !matches!(self, Press::Plain)
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
pub(super) fn drag(landed: &Here, from: [f64; 2]) -> Press {
    match landed.spot {
        _ if landed.carrying => Press::Spread,
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

impl Touch {
    /// A finger lands on the title screen or a menu.
    pub(super) fn land_menu(
        &mut self,
        layout: &Layout,
        at: [f64; 2],
        here: Here,
        out: &mut Vec<Input>,
    ) -> Role {
        let px = [at[0] as f32, at[1] as f32];
        if self.context.mode == Mode::Menu && layout.on(Button::Back, px) {
            out.push(Input::Back);
            return Role::Spent;
        }
        // A desk has one mouse.
        if self.fingers.iter().any(|f| matches!(f.role, Role::Menu(_))) {
            return Role::Spent;
        }
        out.push(Input::Pointer(at));
        Role::Menu(Gesture::Pending { from: at, here })
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
        if let Gesture::Pending { from, here: landed } = *gesture {
            if !left(&landed, from, &here, at, layout.slop()) {
                out.push(Input::Pointer(at));
                return;
            }
            let press = drag(&landed, from);
            out.push(Input::Pointer(from));
            out.push(Input::Press {
                button: press.button(),
                fresh: true,
            });
            *gesture = Gesture::Pressing {
                press,
                outside: None,
            };
        }
        if let Gesture::Pressing { press, outside } = gesture {
            *outside = (press.throws() && here.spot == Spot::Outside).then_some(at);
        }
        out.push(Input::Pointer(at));
    }

    /// A menu finger lifts at `at`, over `here`.
    pub(super) fn lift_menu(
        &mut self,
        gesture: Gesture,
        at: [f64; 2],
        here: Here,
        out: &mut Vec<Input>,
    ) {
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
                click(out, MouseButton::Left, false);
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
            Gesture::Pressing { press, .. } => out.push(Input::Release(press.button())),
        }
        out.push(Input::Pointer(AWAY));
    }

    /// Let go of what a menu finger holds without acting: a cancelled touch, or a screen that
    /// changed under it. Nothing hovers when the button comes up, and its release clicks nothing.
    pub(super) fn let_go(&mut self, gesture: Gesture, out: &mut Vec<Input>) {
        out.push(Input::Pointer(AWAY));
        if let Gesture::Pressing { press, .. } = gesture {
            out.push(Input::Abort(press.button()));
        }
    }

    /// The game's screen changed under the menu fingers while it took their clicks: they do
    /// nothing more until they lift, and their buttons' releases never reach the new screen.
    pub fn spend(&mut self) -> Vec<Input> {
        for finger in &mut self.fingers {
            if let Role::Menu(_) = finger.role {
                finger.role = Role::Spent;
            }
        }
        vec![Input::Pointer(AWAY)]
    }

    /// Where a carried stack is drawn while nothing hovers: where the last finger lifted.
    pub fn rest(&self) -> Option<[f64; 2]> {
        self.menus.rest
    }
}

/// What menus keep between fingers.
#[derive(Debug, Default)]
pub(super) struct Menus {
    /// Where the last menu finger lifted.
    rest: Option<[f64; 2]>,
}

#[cfg(test)]
mod tests {
    use super::super::{Context, Phase, TouchEvent};
    use super::*;
    use std::time::Instant;

    const LEFT: MouseButton = MouseButton::Left;

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
            drag(&slot(3), A),
            Press::Carry {
                origin: origin(slot(3)),
                from: A
            }
        );
        let result = slot_of(200, true, SlotKind::Result);
        assert!(matches!(drag(&result, A), Press::Carry { .. }));
        assert_eq!(drag(&empty(3), A), Press::Plain);
        assert_eq!(
            drag(&slot_of(901, true, SlotKind::Display), A),
            Press::Plain
        );
        assert_eq!(drag(&carrying(empty(3)), A), Press::Spread);
        assert_eq!(drag(&carrying(slot(3)), A), Press::Spread);
        assert_eq!(drag(&carrying(spot(Spot::Outside)), A), Press::Spread);
        assert_eq!(drag(&Here::default(), A), Press::Plain);
    }
}
