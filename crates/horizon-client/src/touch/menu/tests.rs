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

/// A finger's event on what the game showed last frame.
trait On {
    fn on(&mut self, layout: &Layout, event: TouchEvent, now: Instant) -> Vec<Input>;
}

impl On for Touch {
    fn on(&mut self, layout: &Layout, event: TouchEvent, now: Instant) -> Vec<Input> {
        self.touch(layout, self.context, event, now)
    }
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
    touch.on(&layout, event(1, Phase::Start, A, landed), now);
    let moved = touch.on(&layout, event(1, Phase::Move, B, over), now);
    let lifted = touch.on(&layout, event(1, Phase::End, B, over), now);
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
        touch.on(&layout, event(1, Phase::Start, A, plain), now),
        [Input::Pointer(A)]
    );
    assert_eq!(
        touch.on(&layout, event(1, Phase::Move, at, plain), now),
        [Input::Pointer(at)]
    );
    assert_eq!(
        touch.on(&layout, event(1, Phase::End, at, plain), now),
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
    touch.on(&layout, event(1, Phase::Start, A, Here::default()), now);
    assert_eq!(
        touch.on(&layout, event(1, Phase::Cancel, A, Here::default()), now),
        [Input::Pointer(AWAY)]
    );
}

#[test]
fn a_drag_on_a_button_presses_where_it_landed_then_follows() {
    let (mut touch, layout, now) = menu();
    let plain = Here::default();
    touch.on(&layout, event(1, Phase::Start, A, plain), now);
    assert_eq!(
        touch.on(&layout, event(1, Phase::Move, B, plain), now),
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
        touch.on(&layout, event(1, Phase::End, B, plain), now),
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
    touch.on(&layout, event(1, Phase::Start, A, Here::default()), now);
    touch.on(&layout, event(1, Phase::Move, B, Here::default()), now);
    assert_eq!(
        touch.on(&layout, event(1, Phase::Cancel, B, Here::default()), now),
        [Input::Pointer(AWAY), Input::Abort(LEFT)]
    );
}

#[test]
fn leaving_a_menu_mid_drag_lets_go_too() {
    let (mut touch, layout, now) = menu();
    touch.on(&layout, event(1, Phase::Start, A, Here::default()), now);
    touch.on(&layout, event(1, Phase::Move, B, Here::default()), now);
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
            .on(&layout, event(1, Phase::End, B, Here::default()), now)
            .is_empty()
    );
}

#[test]
fn a_tap_that_wanders_inside_its_slot_is_still_a_tap() {
    let (mut touch, layout, now) = menu();
    touch.on(&layout, event(1, Phase::Start, A, slot(3)), now);
    let far = [A[0] + 60.0, A[1]];
    assert_eq!(
        touch.on(&layout, event(1, Phase::Move, far, slot(3)), now),
        [Input::Pointer(far)]
    );
    assert!(matches!(
        touch.on(&layout, event(1, Phase::End, far, slot(3)), now)[..],
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
            Input::PutDown(B),
            Input::PutDown(A),
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
            Input::PutDown(A),
            Input::Pointer(AWAY)
        ]
    );
}

#[test]
fn a_stack_dragged_back_onto_its_slot_stays_in_hand() {
    let (mut touch, layout, now) = menu();
    touch.on(&layout, event(1, Phase::Start, A, slot(3)), now);
    touch.on(&layout, event(1, Phase::Move, B, empty(4)), now);
    touch.on(&layout, event(1, Phase::Move, A, slot(3)), now);
    assert_eq!(
        touch.on(&layout, event(1, Phase::End, A, slot(3)), now),
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
    touch.on(&layout, event(1, Phase::Start, A, slot(3)), now);
    touch.on(&layout, event(1, Phase::Move, B, spot(Spot::Outside)), now);
    assert!(matches!(
        touch.fingers[0].role,
        Role::Menu(Gesture::Pressing {
            outside: Some(B),
            ..
        })
    ));
    assert_eq!(
        touch.on(&layout, event(1, Phase::End, B, spot(Spot::Outside)), now),
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
    let result = slot_of(650, true, SlotKind::Result);
    let (_, lifted) = drag_and_lift(result, slot(4));
    assert_eq!(
        lifted,
        [
            Input::Pointer(B),
            Input::Release(LEFT),
            Input::PutDown(B),
            Input::Pointer(AWAY)
        ]
    );
}

#[test]
fn a_crafted_stack_dropped_on_a_gap_stays_in_hand() {
    let result = slot_of(650, true, SlotKind::Result);
    let (_, lifted) = drag_and_lift(result, Here::default());
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
    touch.on(&layout, event(1, Phase::Start, A, carrying(empty(3))), now);
    let inside = [A[0] + 10.0, A[1]];
    assert_eq!(
        touch.on(
            &layout,
            event(1, Phase::Move, inside, carrying(empty(3))),
            now
        ),
        [Input::Pointer(inside)]
    );
    assert_eq!(
        touch.on(&layout, event(1, Phase::Move, B, carrying(empty(4))), now),
        [Input::Pointer(A), press(true), Input::Pointer(B)]
    );
    assert_eq!(
        touch.on(&layout, event(1, Phase::End, B, carrying(empty(4))), now),
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
    touch.on(&layout, event(1, Phase::Start, A, outside), now);
    assert_eq!(
        touch.on(&layout, event(1, Phase::End, A, outside), now),
        [Input::Pointer(A), Input::Pointer(AWAY)]
    );
    // With nothing in hand there is nothing to throw, and the click is the mouse's.
    touch.on(&layout, event(2, Phase::Start, A, spot(Spot::Outside)), now);
    assert!(
        touch
            .on(&layout, event(2, Phase::End, A, spot(Spot::Outside)), now)
            .contains(&press(false))
    );
}

#[test]
fn a_tap_on_a_field_asks_for_the_keyboard() {
    let (mut touch, layout, now) = menu();
    touch.on(&layout, event(1, Phase::Start, A, spot(Spot::Field)), now);
    assert_eq!(
        touch.on(&layout, event(1, Phase::End, A, spot(Spot::Field)), now),
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
            touch.on(&layout, event(id, Phase::Start, A, slot(3)), now),
            [Input::Pointer(A)]
        );
        assert_eq!(
            touch.on(&layout, event(id, Phase::End, A, slot(3)), now),
            tap
        );
    }
}

#[test]
fn a_drag_right_after_a_tap_on_the_same_slot_is_no_double_click() {
    let (mut touch, layout, now) = menu();
    touch.on(&layout, event(1, Phase::Start, A, slot(3)), now);
    touch.on(&layout, event(1, Phase::End, A, slot(3)), now);
    // The tap picked the stack up; the next finger spreads it from the same slot.
    touch.on(&layout, event(2, Phase::Start, A, carrying(empty(3))), now);
    let moved = touch.on(&layout, event(2, Phase::Move, B, carrying(empty(4))), now);
    assert!(moved.contains(&press(true)) && !moved.contains(&press(false)));
}

/// What the game shows next: the same menu, another scene.
fn next_scene(touch: &Touch) -> Context {
    Context {
        scene: touch.context.scene + 1,
        ..touch.context
    }
}

#[test]
fn a_finger_resting_when_the_scene_changes_clicks_nothing_on_the_next() {
    let (mut touch, layout, now) = menu();
    touch.on(&layout, event(1, Phase::Start, A, Here::default()), now);
    // A second finger's Back leaves the screen, and the first lifts on the one it led to.
    let [x, y, w, h] = layout.button(Button::Back);
    let back = [f64::from(x + w / 2.0), f64::from(y + h / 2.0)];
    assert_eq!(
        touch.on(&layout, event(2, Phase::Start, back, Here::default()), now),
        [Input::Back]
    );
    let next = next_scene(&touch);
    assert_eq!(
        touch.touch(&layout, next, event(1, Phase::End, A, Here::default()), now),
        [Input::Pointer(AWAY)]
    );
    // The next finger is the pointer again.
    assert_eq!(
        touch.on(&layout, event(3, Phase::Start, A, Here::default()), now),
        [Input::Pointer(A)]
    );
}

#[test]
fn a_scene_change_mid_carry_lets_the_button_go_and_puts_nothing_down() {
    let (mut touch, layout, now) = menu();
    touch.on(&layout, event(1, Phase::Start, A, slot(3)), now);
    touch.on(&layout, event(1, Phase::Move, B, empty(4)), now);
    let next = next_scene(&touch);
    assert_eq!(
        touch.frame(next, now),
        [Input::Pointer(AWAY), Input::Abort(LEFT)]
    );
    assert!(
        touch
            .on(&layout, event(1, Phase::End, B, empty(4)), now)
            .is_empty()
    );
}

#[test]
fn a_scene_change_lets_go_of_shift() {
    let (mut touch, layout, now) = inventory();
    let at = shift_button(&layout);
    touch.on(&layout, event(9, Phase::Start, at, Here::default()), now);
    let next = next_scene(&touch);
    assert!(touch.frame(next, now).is_empty());
    assert!(!tap_shifted(&mut touch, &layout, 1, slot(3), now));
    touch.on(&layout, event(9, Phase::End, at, Here::default()), now);
    assert!(!tap_shifted(&mut touch, &layout, 2, slot(3), now));
}

#[test]
fn the_last_lift_is_where_a_carried_stack_rests() {
    let (mut touch, layout, now) = menu();
    assert_eq!(touch.rest(), None);
    touch.on(&layout, event(1, Phase::Start, A, slot(3)), now);
    touch.on(&layout, event(1, Phase::End, B, slot(3)), now);
    assert_eq!(touch.rest(), Some(B));
}

#[test]
fn drag_decides_from_where_it_landed() {
    let press = |here: &Here, shifted| match drag(here, A, B, shifted) {
        Drag::Press(press) => press,
        Drag::Scroll(_) => panic!("{here:?} scrolls"),
    };
    let Spot::Slot(origin) = slot(3).spot else {
        unreachable!()
    };
    assert_eq!(press(&slot(3), false), Press::Carry { origin, from: A });
    let result = slot_of(200, true, SlotKind::Result);
    assert!(matches!(press(&result, false), Press::Carry { .. }));
    assert_eq!(press(&empty(3), false), Press::Plain);
    let copy = slot_of(901, true, SlotKind::Display);
    assert_eq!(press(&copy, false), Press::Plain);
    assert_eq!(press(&carrying(empty(3)), false), Press::Spread);
    assert_eq!(press(&carrying(slot(3)), false), Press::Spread);
    assert_eq!(press(&carrying(spot(Spot::Outside)), false), Press::Spread);
    assert_eq!(press(&Here::default(), false), Press::Plain);
    // Shifted, a stack quick-moves at the press: there is nothing to carry.
    assert_eq!(press(&slot(3), true), Press::Plain);
    assert_eq!(press(&carrying(empty(3)), true), Press::Spread);
}

const RIGHT: MouseButton = MouseButton::Right;

/// The inputs of a finger that lands at `A` over `landed` and is held still past `MENU_HOLD`.
fn long_press(landed: Here) -> (Touch, Layout, Instant, Vec<Input>) {
    let (mut touch, layout, now) = menu();
    touch.on(&layout, event(1, Phase::Start, A, landed), now);
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
        touch.on(&layout, event(1, Phase::End, A, slot(3)), now),
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
        touch.on(&layout, event(1, Phase::End, A, carrying(empty(3))), now),
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
        touch.on(&layout, event(1, Phase::Move, B, carrying(empty(4))), now),
        [Input::Pointer(B)]
    );
    assert_eq!(
        touch.on(&layout, event(1, Phase::End, B, carrying(empty(4))), now),
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
        touch.on(&layout, event(1, Phase::End, A, result), now),
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
        touch.on(&layout, event(1, Phase::End, A, Here::default()), now),
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
    touch.on(&layout, event(1, Phase::Move, B, carrying(empty(4))), now);
    assert_eq!(
        touch.on(&layout, event(1, Phase::Cancel, B, carrying(empty(4))), now),
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
    touch.on(&layout, event(1, Phase::Start, A, slot(3)), now);
    assert_eq!(touch.tooltip_lift(&layout), layout.finger_clearance());
    touch.on(&layout, event(1, Phase::End, A, slot(3)), now);
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
    touch.on(layout, event(id, Phase::Start, A, here), now);
    let out = touch.on(layout, event(id, Phase::End, A, here), now);
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
            .on(layout, event(9, Phase::Start, at, Here::default()), now)
            .is_empty()
    );
    assert!(
        touch
            .on(layout, event(9, Phase::End, at, Here::default()), now)
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
    touch.on(&layout, event(9, Phase::Start, at, Here::default()), now);
    // The finger on Shift leaves the pointer to another one.
    for id in 1..4 {
        assert!(tap_shifted(&mut touch, &layout, id, slot(3), now));
    }
    touch.on(&layout, event(9, Phase::End, at, Here::default()), now);
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
    touch.on(
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
    touch.on(&layout, event(1, Phase::Start, A, slot(3)), now);
    assert_eq!(
        touch.on(&layout, event(1, Phase::Move, B, empty(4)), now),
        [
            Input::Pointer(A),
            Input::Shift(true),
            press(true),
            Input::Pointer(B)
        ]
    );
    assert_eq!(
        touch.on(&layout, event(1, Phase::End, B, empty(4)), now),
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
    touch.on(&layout, event(1, Phase::Start, A, carrying(empty(3))), now);
    touch.on(&layout, event(1, Phase::Move, B, carrying(empty(4))), now);
    assert_eq!(
        touch.on(&layout, event(1, Phase::Cancel, B, carrying(empty(4))), now),
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
        touch.on(&layout, event(1, Phase::Start, at, Here::default()), now),
        [Input::Pointer(at)]
    );
    touch.on(&layout, event(1, Phase::Cancel, at, Here::default()), now);
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

fn listed(here: Here) -> Here {
    Here {
        list: Some(54.0),
        ..here
    }
}

#[test]
fn a_drag_in_a_list_turns_the_wheel_line_by_line() {
    let (mut touch, layout, now) = menu();
    let row = listed(Here::default());
    let to = |dy: f64| [A[0], A[1] + dy];
    touch.on(&layout, event(1, Phase::Start, A, row), now);
    // Out of the slop it scrolls: nothing hovers, and less than a line turns nothing.
    assert_eq!(
        touch.on(&layout, event(1, Phase::Move, to(40.0), row), now),
        [Input::Pointer(AWAY)]
    );
    assert_eq!(
        touch.on(&layout, event(1, Phase::Move, to(120.0), row), now),
        [
            Input::Pointer(A),
            Input::Wheel(1.0),
            Input::Wheel(1.0),
            Input::Pointer(AWAY)
        ]
    );
    assert_eq!(
        touch.on(&layout, event(1, Phase::Move, to(0.0), row), now),
        [
            Input::Pointer(A),
            Input::Wheel(-1.0),
            Input::Wheel(-1.0),
            Input::Pointer(AWAY)
        ]
    );
    assert_eq!(
        touch.on(&layout, event(1, Phase::End, to(0.0), row), now),
        [Input::Pointer(AWAY)]
    );
}

#[test]
fn a_slider_in_a_list_moves_sideways_and_scrolls_upright() {
    let slider = listed(spot(Spot::Slider));
    let sideways = [A[0] + 60.0, A[1] + 10.0];
    let upright = [A[0] + 10.0, A[1] + 60.0];
    assert_eq!(drag(&slider, A, sideways, false), Drag::Press(Press::Plain));
    assert_eq!(drag(&slider, A, upright, false), Drag::Scroll(54.0));
    let field = listed(spot(Spot::Field));
    assert_eq!(drag(&field, A, sideways, false), Drag::Press(Press::Plain));
    // Outside a list a slider is the mouse's drag whichever way it goes.
    assert_eq!(
        drag(&spot(Spot::Slider), A, upright, false),
        Drag::Press(Press::Plain)
    );
}

#[test]
fn a_scrollbar_is_held_even_in_its_list() {
    let bar = listed(spot(Spot::Grab));
    let upright = [A[0], A[1] + 60.0];
    assert_eq!(drag(&bar, A, upright, false), Drag::Press(Press::Plain));
    assert_eq!(
        drag(&spot(Spot::Grab), A, upright, false),
        Drag::Press(Press::Plain)
    );
}

#[test]
fn a_creative_item_dragged_scrolls_the_grid_and_held_gives_a_stack() {
    let copy = listed(slot_of(901, true, SlotKind::Display));
    let upright = [A[0], A[1] + 60.0];
    assert_eq!(drag(&copy, A, upright, false), Drag::Scroll(54.0));
    assert_eq!(drag(&carrying(copy), A, upright, false), Drag::Scroll(54.0));
    let (_, _, _, held) = long_press(copy);
    assert!(held.contains(&Input::Release(MouseButton::Middle)));
}

#[test]
fn a_scroll_leaves_the_shift_latch_for_the_next_click() {
    let (mut touch, layout, now) = inventory();
    tap_shift(&mut touch, &layout, now);
    let row = listed(slot_of(901, true, SlotKind::Display));
    touch.on(&layout, event(1, Phase::Start, A, row), now);
    let moved = touch.on(
        &layout,
        event(1, Phase::Move, [A[0], A[1] + 120.0], row),
        now,
    );
    assert!(!moved.contains(&Input::Shift(true)), "{moved:?}");
    touch.on(&layout, event(1, Phase::End, A, row), now);
    assert!(tap_shifted(&mut touch, &layout, 2, slot(3), now));
}

#[test]
fn wheel_lines_keep_the_remainder_both_ways() {
    let mut rest = 0.0;
    assert_eq!(wheel_lines(&mut rest, 30.0, 20.0), 1);
    assert_eq!(rest, 10.0);
    assert_eq!(wheel_lines(&mut rest, 5.0, 20.0), 0);
    assert_eq!(wheel_lines(&mut rest, 30.0, 20.0), 2);
    assert_eq!(rest, 5.0);
    assert_eq!(wheel_lines(&mut rest, -50.0, 20.0), -2);
    assert_eq!(rest, -5.0);
}
