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

/// A slot; an empty one of the menu's own takes one item from a stack in hand, and a filled one
/// holds another item.
fn slot_of(id: u32, filled: bool, kind: SlotKind) -> Here {
    Here {
        spot: Spot::Slot(SlotSpot {
            id,
            filled,
            kind,
            takes_one: !filled && kind == SlotKind::Normal,
        }),
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

/// The hand holds a stack (`carrying`) or not, from now on.
fn hand(touch: &mut Touch, carrying: bool, now: Instant) {
    let context = Context {
        carrying,
        ..touch.context
    };
    touch.frame(context, now);
}

/// A menu with a stack in hand.
fn holding() -> (Touch, Layout, Instant) {
    let (mut touch, layout, now) = menu();
    hand(&mut touch, true, now);
    (touch, layout, now)
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
        [Input::Abort(LEFT), Input::Pointer(AWAY)]
    );
}

#[test]
fn a_cancelled_slider_drag_comes_up_before_the_pointer_leaves() {
    let (mut touch, layout, now) = menu();
    let slider = listed(spot(Spot::Slider));
    let sideways = [A[0] + 60.0, A[1]];
    touch.on(&layout, event(1, Phase::Start, A, slider), now);
    assert_eq!(
        touch.on(&layout, event(1, Phase::Move, sideways, slider), now),
        [Input::Pointer(A), press(true), Input::Pointer(sideways)]
    );
    assert_eq!(
        touch.on(&layout, event(1, Phase::Cancel, sideways, slider), now),
        [Input::Abort(LEFT), Input::Pointer(AWAY)]
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
        [Input::Abort(LEFT), Input::Pointer(AWAY)]
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
    // The slop is 32 pixels on this layout.
    let layout = Layout::new(2622, 1206, 3.0);
    let left = |landed: &Here, here: &Here, at| left(landed, A, here, at, &layout, false);
    let near = [A[0] + 20.0, A[1]];
    let far = [A[0] + 40.0, A[1]];
    let plain = Here::default();
    assert!(!left(&plain, &plain, near));
    assert!(left(&plain, &plain, far));
    assert!(!left(&slot(3), &slot(3), far));
    assert!(left(&slot(3), &slot(4), near));
    assert!(left(&slot(3), &plain, near));
    // In a list, a slot is a row like any other: the slop decides.
    let listed = Here {
        list: Some(54.0),
        ..slot(3)
    };
    assert!(!left(&listed, &listed, near));
    assert!(left(&listed, &listed, far));
}

/// A slot's pitch, 18 GUI pixels, at the test layout's GUI scale of 5.
const PITCH: f64 = 90.0;

#[test]
fn a_carried_tap_that_rolls_into_a_gap_still_puts_the_stack_on_its_slot() {
    let gap = [A[0] + 45.0, A[1] + 10.0];
    let (mut touch, layout, now) = holding();
    touch.on(&layout, event(1, Phase::Start, A, empty(3)), now);
    assert_eq!(
        touch.on(&layout, event(1, Phase::Move, gap, Here::default()), now),
        [Input::Pointer(gap)]
    );
    assert_eq!(
        touch.on(&layout, event(1, Phase::End, gap, Here::default()), now),
        [
            Input::Pointer(A),
            press(false),
            Input::Release(LEFT),
            Input::Pointer(AWAY)
        ]
    );
    // With an empty hand the gap is off the slot: the stack is dragged.
    let (mut touch, layout, now) = menu();
    touch.on(&layout, event(1, Phase::Start, A, slot(3)), now);
    assert_eq!(
        touch.on(&layout, event(1, Phase::Move, gap, Here::default()), now),
        [Input::Pointer(A), press(true), Input::Pointer(gap)]
    );
}

#[test]
fn a_carried_stack_spreads_once_the_finger_really_leaves_its_slot() {
    let background = [A[0] + PITCH + 10.0, A[1]];
    let near = [A[0] + 20.0, A[1]];
    for (to, over) in [
        (background, Here::default()),
        (near, spot(Spot::Outside)),
        (near, empty(4)),
    ] {
        let (mut touch, layout, now) = holding();
        touch.on(&layout, event(1, Phase::Start, A, empty(3)), now);
        assert_eq!(
            touch.on(&layout, event(1, Phase::Move, to, over), now),
            [Input::Pointer(A), press(true), Input::Pointer(to)],
            "{over:?}"
        );
    }
}

#[test]
fn a_carried_tap_lifted_beside_the_trash_is_not_shifted_by_the_latch() {
    let (mut touch, layout, now) = inventory();
    hand(&mut touch, true, now);
    tap_shift(&mut touch, &layout, now);
    let trash = slot_of(TRASH_ID, false, SlotKind::Trash);
    let gap = [A[0] + 45.0, A[1]];
    touch.on(&layout, event(1, Phase::Start, A, trash), now);
    touch.on(&layout, event(1, Phase::Move, gap, Here::default()), now);
    assert_eq!(
        touch.on(&layout, event(1, Phase::End, gap, Here::default()), now),
        [
            Input::Pointer(A),
            press(false),
            Input::Release(LEFT),
            Input::Pointer(AWAY)
        ]
    );
    assert!(tap_shifted(&mut touch, &layout, 2, slot(3), now));
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
fn a_stack_lifted_over_a_result_is_not_put_down_there_but_goes_back() {
    for filled in [false, true] {
        let (_, lifted) = drag_and_lift(slot(3), slot_of(200, filled, SlotKind::Result));
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
    let (mut touch, layout, now) = holding();
    touch.on(&layout, event(1, Phase::Start, A, empty(3)), now);
    let inside = [A[0] + 10.0, A[1]];
    assert_eq!(
        touch.on(&layout, event(1, Phase::Move, inside, empty(3)), now),
        [Input::Pointer(inside)]
    );
    assert_eq!(
        touch.on(&layout, event(1, Phase::Move, B, empty(4)), now),
        [Input::Pointer(A), press(true), Input::Pointer(B)]
    );
    assert_eq!(
        touch.on(&layout, event(1, Phase::End, B, empty(4)), now),
        [
            Input::Pointer(B),
            Input::Release(LEFT),
            Input::Pointer(AWAY)
        ]
    );
}

#[test]
fn a_tap_outside_with_a_stack_in_hand_throws_nothing() {
    let (mut touch, layout, now) = holding();
    let outside = spot(Spot::Outside);
    touch.on(&layout, event(1, Phase::Start, A, outside), now);
    assert_eq!(
        touch.on(&layout, event(1, Phase::End, A, outside), now),
        [Input::Pointer(A), Input::Pointer(AWAY)]
    );
    // With nothing in hand there is nothing to throw, and the click is the mouse's.
    hand(&mut touch, false, now);
    touch.on(&layout, event(2, Phase::Start, A, spot(Spot::Outside)), now);
    assert!(
        touch
            .on(&layout, event(2, Phase::End, A, spot(Spot::Outside)), now)
            .contains(&press(false))
    );
}

#[test]
fn a_tap_clicks_beyond_the_panel_by_where_it_lifts() {
    let (mut touch, layout, now) = holding();
    let near = [A[0] + 10.0, A[1]];
    let (inside, outside) = (Here::default(), spot(Spot::Outside));
    touch.on(&layout, event(1, Phase::Start, A, inside), now);
    touch.on(&layout, event(1, Phase::Move, near, outside), now);
    assert_eq!(
        touch.on(&layout, event(1, Phase::End, near, outside), now),
        [Input::Pointer(near), Input::Pointer(AWAY)]
    );
    // Landed beyond the panel and lifted inside it, it clicks inside.
    touch.on(&layout, event(2, Phase::Start, A, outside), now);
    touch.on(&layout, event(2, Phase::Move, near, inside), now);
    assert!(
        touch
            .on(&layout, event(2, Phase::End, near, inside), now)
            .contains(&press(false))
    );
}

#[test]
fn a_tap_that_lifts_on_the_trash_is_not_shifted_by_the_latch() {
    let (mut touch, layout, now) = inventory();
    tap_shift(&mut touch, &layout, now);
    let trash = slot_of(TRASH_ID, false, SlotKind::Trash);
    let near = [A[0] + 10.0, A[1]];
    touch.on(&layout, event(1, Phase::Start, A, Here::default()), now);
    touch.on(&layout, event(1, Phase::Move, near, trash), now);
    assert_eq!(
        touch.on(&layout, event(1, Phase::End, near, trash), now),
        [
            Input::Pointer(near),
            press(false),
            Input::Release(LEFT),
            Input::Pointer(AWAY)
        ]
    );
    assert!(tap_shifted(&mut touch, &layout, 2, slot(3), now));
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
    hand(&mut touch, true, now);
    touch.on(&layout, event(2, Phase::Start, A, empty(3)), now);
    let moved = touch.on(&layout, event(2, Phase::Move, B, empty(4)), now);
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
        [Input::Abort(LEFT), Input::Pointer(AWAY)]
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
    let press = |here: &Here, carrying, shifted| match drag(here, carrying, A, B, shifted) {
        Drag::Press(press) => press,
        Drag::Scroll(_) => panic!("{here:?} scrolls"),
    };
    let Spot::Slot(origin) = slot(3).spot else {
        unreachable!()
    };
    assert_eq!(
        press(&slot(3), false, false),
        Press::Carry { origin, from: A }
    );
    let result = slot_of(200, true, SlotKind::Result);
    assert!(matches!(press(&result, false, false), Press::Carry { .. }));
    assert_eq!(press(&empty(3), false, false), Press::Plain);
    let copy = slot_of(901, true, SlotKind::Display);
    assert_eq!(press(&copy, false, false), Press::Plain);
    assert_eq!(press(&empty(3), true, false), Press::Spread);
    assert_eq!(press(&slot(3), true, false), Press::Spread);
    assert_eq!(press(&spot(Spot::Outside), true, false), Press::Spread);
    assert_eq!(press(&Here::default(), false, false), Press::Plain);
    // Shifted, a stack quick-moves at the press: there is nothing to carry.
    assert_eq!(press(&slot(3), false, true), Press::Plain);
    assert_eq!(press(&empty(3), true, true), Press::Spread);
}

const RIGHT: MouseButton = MouseButton::Right;

/// The inputs of a finger that lands at `A` over `landed` and is held still past `MENU_HOLD`,
/// on a menu as `start` makes it.
fn long_press(
    start: (Touch, Layout, Instant),
    landed: Here,
) -> (Touch, Layout, Instant, Vec<Input>) {
    let (mut touch, layout, now) = start;
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
    let (mut touch, layout, now, held) = long_press(menu(), slot(3));
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
fn a_long_press_beyond_the_panel_drops_one_at_once_and_no_more() {
    let (mut touch, layout, now, held) = long_press(holding(), spot(Spot::Outside));
    assert_eq!(
        held,
        [Input::Pointer(A), right_down(), Input::Release(RIGHT)]
    );
    assert!(tick(&mut touch, now + Duration::from_secs(5)).is_empty());
    assert_eq!(
        touch.on(&layout, event(1, Phase::End, A, spot(Spot::Outside)), now),
        [Input::Pointer(A), Input::Pointer(AWAY)]
    );
}

#[test]
fn a_long_press_on_the_result_takes_one_craft_at_once() {
    let result = slot_of(200, true, SlotKind::Result);
    let (mut touch, layout, now, held) = long_press(menu(), result);
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
    let (_, _, _, held) = long_press(menu(), slot_of(901, true, SlotKind::Display));
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
    let (_, _, _, held) = long_press(menu(), spot(Spot::Secondary));
    assert_eq!(
        held,
        [Input::Pointer(A), right_down(), Input::Release(RIGHT)]
    );
}

#[test]
fn a_long_press_on_a_plain_button_keeps_hovering_and_still_taps() {
    let (mut touch, layout, now, held) = long_press(menu(), Here::default());
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
fn hold_decides_from_where_it_landed() {
    let hold = |here: &Here, carrying| hold(here, carrying, false);
    assert_eq!(hold(&slot(3), false), Hold::Press(Press::Split));
    assert_eq!(hold(&empty(3), false), Hold::Nothing);
    assert_eq!(hold(&empty(3), true), Hold::Paint(3));
    assert_eq!(hold(&slot(3), true), Hold::Paint(3));
    assert_eq!(hold(&spot(Spot::Outside), true), Hold::Click(RIGHT));
    assert_eq!(hold(&spot(Spot::Outside), false), Hold::Nothing);
    let result = slot_of(200, true, SlotKind::Result);
    assert_eq!(hold(&result, false), Hold::Click(RIGHT));
    assert_eq!(hold(&result, true), Hold::Done);
    assert_eq!(
        hold(&slot_of(200, false, SlotKind::Result), false),
        Hold::Nothing
    );
    let copy = slot_of(901, true, SlotKind::Display);
    assert_eq!(hold(&copy, false), Hold::Click(MouseButton::Middle));
    assert_eq!(hold(&copy, true), Hold::Done);
    let trash = slot_of(7, false, SlotKind::Trash);
    assert_eq!(hold(&trash, false), Hold::Nothing);
    assert_eq!(hold(&trash, true), Hold::Done);
    assert_eq!(hold(&spot(Spot::Secondary), false), Hold::Click(RIGHT));
    for plain in [Spot::Plain, Spot::Slider, Spot::Grab, Spot::Field] {
        assert_eq!(hold(&spot(plain), false), Hold::Nothing);
    }
}

/// A frame of the menu as it is, at `now`.
fn tick(touch: &mut Touch, now: Instant) -> Vec<Input> {
    let context = touch.context;
    touch.frame(context, now)
}

fn ms(now: Instant, millis: u64) -> Instant {
    now + Duration::from_millis(millis)
}

/// Finger 1's item put down on slot `slot`, at `at`.
const fn place(slot: u32, at: [f64; 2]) -> Input {
    Input::PlaceOne(PlaceOne {
        finger: 1,
        slot,
        at,
    })
}

/// Finger 1 with a stack in hand landed at `A` on empty slot 3, held still for its first item,
/// which the game placed at the time returned.
fn painting() -> (Touch, Layout, Instant) {
    let (mut touch, layout, now) = holding();
    touch.on(&layout, event(1, Phase::Start, A, empty(3)), now);
    assert_eq!(tick(&mut touch, ms(now, 250)), [place(3, A)]);
    touch.placed(1, PlaceResult::Placed);
    (touch, layout, ms(now, 250))
}

#[test]
fn a_quick_tap_with_a_stack_in_hand_puts_it_all_down() {
    let (mut touch, layout, now) = holding();
    touch.on(&layout, event(1, Phase::Start, A, empty(3)), now);
    assert!(tick(&mut touch, ms(now, 249)).is_empty());
    assert_eq!(
        touch.on(&layout, event(1, Phase::End, A, empty(3)), ms(now, 249)),
        [
            Input::Pointer(A),
            press(false),
            Input::Release(LEFT),
            Input::Pointer(AWAY)
        ]
    );
}

#[test]
fn a_held_slot_takes_one_at_once_then_one_after_a_dwell_then_one_steadily() {
    let (mut touch, layout, now) = holding();
    touch.on(&layout, event(1, Phase::Start, A, empty(3)), now);
    let mut placed = Vec::new();
    for millis in (0..=1500).step_by(10) {
        if tick(&mut touch, ms(now, millis)) == [place(3, A)] {
            touch.placed(1, PlaceResult::Placed);
            placed.push(millis);
        }
    }
    assert_eq!(placed, [250, 700, 900, 1100, 1300, 1500]);
    assert_eq!(
        touch.on(&layout, event(1, Phase::End, A, empty(3)), ms(now, 1500)),
        [Input::Pointer(A), Input::Pointer(AWAY)]
    );
}

#[test]
fn a_late_frame_places_one_item_not_a_burst() {
    let (mut touch, _, now) = painting();
    let late = ms(now, 3000);
    assert_eq!(tick(&mut touch, late), [place(3, A)]);
    touch.placed(1, PlaceResult::Placed);
    assert!(tick(&mut touch, late).is_empty());
    assert!(tick(&mut touch, ms(late, 199)).is_empty());
    assert_eq!(tick(&mut touch, ms(late, 200)), [place(3, A)]);
}

#[test]
fn one_item_is_asked_for_at_a_time() {
    let (mut touch, layout, now) = holding();
    touch.on(&layout, event(1, Phase::Start, A, empty(3)), now);
    assert_eq!(tick(&mut touch, ms(now, 250)), [place(3, A)]);
    assert!(tick(&mut touch, ms(now, 5000)).is_empty());
    assert_eq!(
        touch.on(&layout, event(1, Phase::Move, B, empty(4)), ms(now, 5000)),
        [Input::Pointer(B)]
    );
}

#[test]
fn a_slide_puts_one_on_each_new_slot_and_a_revisited_slot_waits_for_the_dwell() {
    let (mut touch, layout, now) = painting();
    let c = [B[0] + 100.0, B[1]];
    assert_eq!(
        touch.on(&layout, event(1, Phase::Move, B, empty(4)), now),
        [place(4, B), Input::Pointer(B)]
    );
    touch.placed(1, PlaceResult::Placed);
    assert_eq!(
        touch.on(&layout, event(1, Phase::Move, c, empty(5)), now),
        [place(5, c), Input::Pointer(c)]
    );
    touch.placed(1, PlaceResult::Placed);
    let back = [A[0] + 10.0, A[1]];
    assert_eq!(
        touch.on(&layout, event(1, Phase::Move, back, slot(3)), now),
        [Input::Pointer(back)]
    );
    assert!(tick(&mut touch, ms(now, 449)).is_empty());
    assert_eq!(tick(&mut touch, ms(now, 450)), [place(3, back)]);
}

#[test]
fn a_gap_keeps_the_slot_and_a_pitch_of_background_lets_it_go() {
    let (mut touch, layout, now) = painting();
    let gap = [A[0] + 45.0, A[1]];
    assert_eq!(
        touch.on(&layout, event(1, Phase::Move, gap, Here::default()), now),
        [Input::Pointer(gap)]
    );
    assert_eq!(tick(&mut touch, ms(now, 450)), [place(3, A)]);
    touch.placed(1, PlaceResult::Placed);
    let far = [A[0] + PITCH + 10.0, A[1]];
    touch.on(&layout, event(1, Phase::Move, far, Here::default()), now);
    assert!(tick(&mut touch, ms(now, 5000)).is_empty());
    assert_eq!(
        touch.on(&layout, event(1, Phase::Move, B, empty(4)), ms(now, 5000)),
        [place(4, B), Input::Pointer(B)]
    );
}

#[test]
fn an_ended_painting_puts_nothing_more_down() {
    let (mut touch, layout, now) = painting();
    assert_eq!(tick(&mut touch, ms(now, 450)), [place(3, A)]);
    touch.placed(1, PlaceResult::Ended);
    assert!(tick(&mut touch, ms(now, 5000)).is_empty());
    assert_eq!(
        touch.on(&layout, event(1, Phase::Move, B, empty(4)), now),
        [Input::Pointer(B)]
    );
    assert_eq!(
        touch.on(&layout, event(1, Phase::End, B, empty(4)), now),
        [Input::Pointer(B), Input::Pointer(AWAY)]
    );
}

#[test]
fn a_painting_finger_stops_once_the_hand_is_empty() {
    let (mut touch, layout, now) = painting();
    hand(&mut touch, false, now);
    hand(&mut touch, true, now);
    assert!(tick(&mut touch, ms(now, 5000)).is_empty());
    assert_eq!(
        touch.on(&layout, event(1, Phase::Move, B, empty(4)), now),
        [Input::Pointer(B)]
    );
}

#[test]
fn a_blocked_slot_takes_no_more_and_its_lift_clicks_nothing() {
    let (mut touch, layout, now) = holding();
    touch.on(&layout, event(1, Phase::Start, A, slot(3)), now);
    assert_eq!(tick(&mut touch, ms(now, 250)), [place(3, A)]);
    touch.placed(1, PlaceResult::Blocked);
    assert!(tick(&mut touch, ms(now, 5000)).is_empty());
    let back = [A[0] + 10.0, A[1]];
    touch.on(&layout, event(1, Phase::Move, B, empty(4)), now);
    touch.placed(1, PlaceResult::Placed);
    assert_eq!(
        touch.on(&layout, event(1, Phase::Move, back, slot(3)), now),
        [Input::Pointer(back)]
    );
    assert!(tick(&mut touch, ms(now, 5000)).is_empty());
    assert_eq!(
        touch.on(&layout, event(1, Phase::End, back, slot(3)), now),
        [Input::Pointer(back), Input::Pointer(AWAY)]
    );
}

/// How many of the touch layer's quads lie within a ring's reach of `at`.
fn marks_near(touch: &Touch, layout: &Layout, at: [f64; 2], now: Instant) -> usize {
    let atlas =
        crate::hud::HudAtlas::build(std::path::Path::new("/nonexistent/horizon-test-assets"))
            .unwrap();
    let mut batch = crate::hud::HudBatch::new([2622, 1206]);
    touch.draw(&mut batch, &atlas, layout, false, now);
    let reach = 14.0 * layout.unit / layout.gui;
    let (x, y) = (at[0] as f32 / layout.gui, at[1] as f32 / layout.gui);
    batch
        .vertices
        .iter()
        .filter(|v| (v.pos[0] - x).hypot(v.pos[1] - y) < reach)
        .count()
}

#[test]
fn the_ring_fills_toward_the_next_item_and_not_over_a_blocked_slot() {
    let (mut touch, layout, now) = holding();
    touch.on(&layout, event(1, Phase::Start, A, empty(3)), now);
    assert!(marks_near(&touch, &layout, A, ms(now, 100)) > 0);
    assert_eq!(tick(&mut touch, ms(now, 250)), [place(3, A)]);
    touch.placed(1, PlaceResult::Placed);
    assert!(marks_near(&touch, &layout, A, ms(now, 400)) > 0);
    let (mut touch, layout, now) = holding();
    touch.on(&layout, event(1, Phase::Start, A, slot(3)), now);
    tick(&mut touch, ms(now, 250));
    touch.placed(1, PlaceResult::Blocked);
    assert_eq!(marks_near(&touch, &layout, A, ms(now, 400)), 0);
}

#[test]
fn the_first_ring_fills_only_where_an_item_will_land() {
    let (mut touch, layout, now) = holding();
    let refuses = Here {
        spot: Spot::Slot(SlotSpot {
            id: 3,
            filled: false,
            kind: SlotKind::Normal,
            takes_one: false,
        }),
        ..Here::default()
    };
    touch.on(&layout, event(1, Phase::Start, A, refuses), now);
    assert_eq!(marks_near(&touch, &layout, A, ms(now, 100)), 0);
    assert_eq!(tick(&mut touch, ms(now, 250)), [place(3, A)]);
    touch.placed(1, PlaceResult::Blocked);
    assert_eq!(
        touch.on(&layout, event(1, Phase::End, A, refuses), ms(now, 300)),
        [Input::Pointer(A), Input::Pointer(AWAY)]
    );
    let (mut touch, layout, now) = holding();
    touch.on(&layout, event(1, Phase::Start, A, slot(3)), now);
    assert_eq!(
        marks_near(&touch, &layout, A, ms(now, 100)),
        0,
        "another item"
    );
}

#[test]
fn the_loupe_shows_the_painted_slot_above_the_fingertip() {
    let (mut touch, layout, now) = holding();
    touch.on(&layout, event(1, Phase::Start, A, empty(3)), now);
    assert_eq!(touch.loupe(&layout), None);
    let (mut touch, layout, now) = painting();
    let gap = [A[0] + 45.0, A[1]];
    touch.on(&layout, event(1, Phase::Move, gap, Here::default()), now);
    let loupe = touch.loupe(&layout).unwrap();
    assert_eq!(loupe.slot, 3);
    let [x, y, w, h] = loupe.square;
    assert!(
        x < gap[0] as f32 && gap[0] as f32 <= x + w,
        "over the finger: {loupe:?}"
    );
    assert!(
        y + h <= gap[1] as f32 - layout.finger_clearance(),
        "clear of the fingertip: {loupe:?}"
    );
    assert!(w >= 18.0 * layout.gui, "an item fits: {loupe:?}");
    tick(&mut touch, ms(now, 450));
    touch.placed(1, PlaceResult::Ended);
    assert_eq!(touch.loupe(&layout), None);
}

#[test]
fn shift_at_the_landing_keeps_the_shifted_tap_and_paints_nothing() {
    let (mut touch, layout, now) = inventory();
    hand(&mut touch, true, now);
    tap_shift(&mut touch, &layout, now);
    touch.on(&layout, event(1, Phase::Start, A, empty(3)), now);
    assert!(tick(&mut touch, ms(now, 250)).is_empty());
    assert!(tick(&mut touch, ms(now, 900)).is_empty());
    assert_eq!(
        touch.on(&layout, event(1, Phase::End, A, empty(3)), ms(now, 900)),
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

#[test]
fn shift_during_painting_ends_it() {
    let (mut touch, layout, now) = inventory();
    hand(&mut touch, true, now);
    touch.on(&layout, event(1, Phase::Start, A, empty(3)), now);
    assert_eq!(tick(&mut touch, ms(now, 250)), [place(3, A)]);
    touch.placed(1, PlaceResult::Placed);
    tap_shift(&mut touch, &layout, ms(now, 300));
    assert_eq!(
        touch.on(&layout, event(1, Phase::Move, B, empty(4)), ms(now, 300)),
        [Input::Pointer(B)]
    );
    assert!(tick(&mut touch, ms(now, 5000)).is_empty());
    assert_eq!(
        touch.on(&layout, event(1, Phase::End, B, empty(4)), ms(now, 5000)),
        [Input::Pointer(B), Input::Pointer(AWAY)]
    );
}

#[test]
fn a_carried_hold_on_a_result_a_copy_or_the_trash_does_nothing_even_at_its_lift() {
    for landed in [
        slot_of(200, true, SlotKind::Result),
        slot_of(901, true, SlotKind::Display),
        slot_of(TRASH_ID, false, SlotKind::Trash),
    ] {
        let (mut touch, layout, now) = holding();
        touch.on(&layout, event(1, Phase::Start, A, landed), now);
        assert!(tick(&mut touch, ms(now, 250)).is_empty(), "{landed:?}");
        assert!(tick(&mut touch, ms(now, 800)).is_empty(), "{landed:?}");
        assert_eq!(
            touch.on(&layout, event(1, Phase::End, A, landed), ms(now, 800)),
            [Input::Pointer(A), Input::Pointer(AWAY)],
            "{landed:?}"
        );
        // A quick tap there is the mouse's click.
        touch.on(&layout, event(2, Phase::Start, A, landed), now);
        assert!(
            touch
                .on(&layout, event(2, Phase::End, A, landed), ms(now, 100))
                .contains(&press(false))
        );
    }
}

#[test]
fn a_painting_finger_has_nothing_to_undo_when_cancelled_or_the_scene_changes() {
    let (mut touch, layout, now) = painting();
    assert_eq!(
        touch.on(&layout, event(1, Phase::Cancel, A, empty(3)), now),
        [Input::Pointer(AWAY)]
    );
    let (mut touch, layout, now) = painting();
    let next = next_scene(&touch);
    assert_eq!(touch.frame(next, now), [Input::Pointer(AWAY)]);
    assert!(tick(&mut touch, ms(now, 5000)).is_empty());
    assert!(
        touch
            .on(&layout, event(1, Phase::End, A, empty(3)), now)
            .is_empty()
    );
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
    hand(&mut touch, true, now);
    tap_shift(&mut touch, &layout, now);
    touch.on(&layout, event(1, Phase::Start, A, empty(3)), now);
    touch.on(&layout, event(1, Phase::Move, B, empty(4)), now);
    assert_eq!(
        touch.on(&layout, event(1, Phase::Cancel, B, empty(4)), now),
        [
            Input::Abort(LEFT),
            Input::Shift(false),
            Input::Pointer(AWAY)
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
    assert_eq!(
        drag(&slider, false, A, sideways, false),
        Drag::Press(Press::Plain)
    );
    assert_eq!(drag(&slider, false, A, upright, false), Drag::Scroll(54.0));
    let field = listed(spot(Spot::Field));
    assert_eq!(
        drag(&field, false, A, sideways, false),
        Drag::Press(Press::Plain)
    );
    // Outside a list a slider is the mouse's drag whichever way it goes.
    assert_eq!(
        drag(&spot(Spot::Slider), false, A, upright, false),
        Drag::Press(Press::Plain)
    );
}

#[test]
fn a_scrollbar_is_held_even_in_its_list() {
    let bar = listed(spot(Spot::Grab));
    let upright = [A[0], A[1] + 60.0];
    assert_eq!(
        drag(&bar, false, A, upright, false),
        Drag::Press(Press::Plain)
    );
    assert_eq!(
        drag(&spot(Spot::Grab), false, A, upright, false),
        Drag::Press(Press::Plain)
    );
}

#[test]
fn a_creative_item_dragged_scrolls_the_grid_and_held_gives_a_stack() {
    let copy = listed(slot_of(901, true, SlotKind::Display));
    let upright = [A[0], A[1] + 60.0];
    assert_eq!(drag(&copy, false, A, upright, false), Drag::Scroll(54.0));
    assert_eq!(drag(&copy, true, A, upright, false), Drag::Scroll(54.0));
    let (_, _, _, held) = long_press(menu(), copy);
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
