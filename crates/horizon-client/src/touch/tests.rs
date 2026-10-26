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
            self.context,
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
        hud_visible: true,
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
fn the_chat_button_presses_key_chat() {
    let (mut touch, layout, now) = playing(Movement::DPad);
    let chat = centre(layout.button(Button::Chat));
    assert_eq!(
        touch.finger(&layout, 1, Phase::Start, chat, now),
        [Input::Mapping("key.chat", true)]
    );
    assert!(touch.finger(&layout, 1, Phase::End, chat, now).is_empty());
    let context = touch.context;
    assert_eq!(
        touch.frame(context, now + MIN_STATE),
        [Input::Mapping("key.chat", false)]
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
fn the_hidden_hotbar_answers_no_taps_and_hiding_it_ends_a_finger_on_it() {
    let (mut touch, layout, now) = playing(Movement::DPad);
    let slot = |i: f64| [(171.0 + 1.0 + 20.0 * i + 10.0) * 5.0, 230.0 * 5.0];
    touch.finger(&layout, 1, Phase::Start, slot(0.0), now);
    let hidden = Context {
        hud_visible: false,
        ..touch.context
    };
    assert!(touch.frame(hidden, now).is_empty());
    assert!(
        touch
            .finger(&layout, 1, Phase::Move, slot(4.0), now)
            .is_empty()
    );
    touch.finger(&layout, 1, Phase::End, slot(4.0), now);
    // A tap where the hotbar was is a tap on the view.
    assert!(
        touch
            .finger(&layout, 2, Phase::Start, slot(2.0), now)
            .is_empty()
    );
    assert_eq!(
        touch.finger(&layout, 2, Phase::End, slot(2.0), now),
        [Input::Tap]
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
