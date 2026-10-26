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
    Pressing { button: MouseButton },
}

/// The finger has left where it landed: its slot, for a slot outside a list (a tap may wander
/// inside a slot); its slop anywhere else.
pub(super) fn left(landed: &Here, from: [f64; 2], here: &Here, at: [f64; 2], slop: f32) -> bool {
    match (landed.spot, landed.list) {
        (Spot::Slot(s), None) => !matches!(here.spot, Spot::Slot(t) if t.id == s.id),
        _ => ((at[0] - from[0]) as f32).hypot((at[1] - from[1]) as f32) > slop,
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
        match *gesture {
            Gesture::Pending { from, here: landed } => {
                if left(&landed, from, &here, at, layout.slop()) {
                    out.push(Input::Pointer(from));
                    out.push(Input::Press {
                        button: MouseButton::Left,
                        fresh: true,
                    });
                    *gesture = Gesture::Pressing {
                        button: MouseButton::Left,
                    };
                }
                out.push(Input::Pointer(at));
            }
            Gesture::Pressing { .. } => out.push(Input::Pointer(at)),
        }
    }

    /// A menu finger lifts at `at`.
    pub(super) fn lift_menu(&mut self, gesture: Gesture, at: [f64; 2], out: &mut Vec<Input>) {
        out.push(Input::Pointer(at));
        match gesture {
            // Two taps on one slot reach the game as two clicks at their own lifts, so its double
            // click (PICKUP_ALL) is the game's to decide, on its own clock.
            Gesture::Pending { .. } => click(out, MouseButton::Left, false),
            Gesture::Pressing { button } => out.push(Input::Release(button)),
        }
        out.push(Input::Pointer(AWAY));
    }

    /// Let go of what a menu finger holds without acting: a cancelled touch, or a screen that
    /// changed under it. Nothing hovers when the button comes up, and its release clicks nothing.
    pub(super) fn let_go(&mut self, gesture: Gesture, out: &mut Vec<Input>) {
        out.push(Input::Pointer(AWAY));
        if let Gesture::Pressing { button } = gesture {
            out.push(Input::Abort(button));
        }
    }
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

    fn slot(id: u32) -> Here {
        Here {
            spot: Spot::Slot(SlotSpot {
                id,
                filled: true,
                kind: SlotKind::Normal,
            }),
            ..Here::default()
        }
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
}
