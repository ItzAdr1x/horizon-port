//! The keys a desk's keyboard has and a touch screen lacks. In play, key caps in the top-left
//! corner press the frequent ones in one tap (F5, F3), and Fn opens a panel of the others, each a
//! tile naming its keys and what they do. A control acts when the finger that landed on it lifts
//! on it, and presses and releases its keys whole, so no key stays down across a screen change.
//!
//! The panel covers play: a finger on a tile taps it and closes the panel, and a finger landing
//! beyond the panel (Back, top left, is there) closes it.

use super::layout::{Layout, contains};
use super::{Input, Mode, Role, Touch};
use std::time::Instant;

/// What a touch control presses: a key, or a debug combination.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Function {
    /// F5: the next camera perspective.
    Perspective,
    /// F3 alone: the debug screen.
    DebugScreen,
    /// F1: the HUD hidden, or shown again.
    HideHud,
    /// F2.
    Screenshot,
    /// F3+B.
    Hitboxes,
    /// F3+G.
    ChunkBorders,
    /// F3+H.
    AdvancedTooltips,
    /// F3+F4: the game mode switcher.
    GameModes,
}

/// Where a control is: one of play's key caps from the left, or one of the panel's tiles, left
/// to right and then down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Place {
    Cap(u8),
    Tile(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tap {
    Press(Function),
    /// Opens the panel.
    Panel,
}

#[derive(Debug)]
pub(super) struct Control {
    pub(super) place: Place,
    /// The keys as the keyboard names them: all a key cap shows.
    pub(super) keys: &'static str,
    /// What they do: under the keys on a tile.
    pub(super) does: &'static str,
    pub(super) tap: Tap,
}

const fn control(place: Place, keys: &'static str, does: &'static str, tap: Tap) -> Control {
    Control {
        place,
        keys,
        does,
        tap,
    }
}

/// Every control: the one table that places, draws and hit-tests them.
pub(super) const CONTROLS: [Control; 9] = [
    control(
        Place::Cap(0),
        "F5",
        "Camera",
        Tap::Press(Function::Perspective),
    ),
    control(
        Place::Cap(1),
        "F3",
        "Debug screen",
        Tap::Press(Function::DebugScreen),
    ),
    control(Place::Cap(2), "Fn", "More keys", Tap::Panel),
    control(
        Place::Tile(0),
        "F1",
        "Hide HUD",
        Tap::Press(Function::HideHud),
    ),
    control(
        Place::Tile(1),
        "F2",
        "Screenshot",
        Tap::Press(Function::Screenshot),
    ),
    control(
        Place::Tile(2),
        "F3+B",
        "Hitboxes",
        Tap::Press(Function::Hitboxes),
    ),
    control(
        Place::Tile(3),
        "F3+G",
        "Chunk borders",
        Tap::Press(Function::ChunkBorders),
    ),
    control(
        Place::Tile(4),
        "F3+H",
        "Advanced tooltips",
        Tap::Press(Function::AdvancedTooltips),
    ),
    control(
        Place::Tile(5),
        "F3+F4",
        "Game mode",
        Tap::Press(Function::GameModes),
    ),
];

/// A control a tap reaches now, for agents (`ui`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shown {
    pub keys: &'static str,
    pub does: &'static str,
    /// Physical pixels.
    pub rect: [f32; 4],
}

impl Touch {
    /// The controls a finger can land on: play's key caps, or the panel's tiles while it is open.
    pub(super) fn live_controls(&self) -> impl Iterator<Item = &'static Control> {
        let open = self.panel;
        CONTROLS
            .iter()
            .filter(move |c| matches!(c.place, Place::Tile(_)) == open)
    }

    /// The key caps, or the open panel's tiles, where they are: in play only.
    pub fn functions(&self, layout: &Layout) -> Vec<Shown> {
        if self.context.mode != Mode::Playing {
            return Vec::new();
        }
        self.live_controls()
            .map(|c| Shown {
                keys: c.keys,
                does: c.does,
                rect: layout.control(c.place),
            })
            .collect()
    }

    /// A finger landing in play at `at`: on a control, or anywhere while the panel is open,
    /// where it does nothing else. Beyond the panel, it closes it.
    pub(super) fn land_function(
        &mut self,
        layout: &Layout,
        at: [f32; 2],
        now: Instant,
        out: &mut Vec<Input>,
    ) -> Option<Role> {
        if let Some(control) = self
            .live_controls()
            .find(|c| contains(layout.control(c.place), at))
        {
            return Some(Role::Function(control));
        }
        if !self.panel {
            return None;
        }
        if !contains(layout.panel(), at) {
            self.panel = false;
            self.spend_fingers(now, out);
        }
        Some(Role::Spent)
    }

    /// A finger that landed on `control` lifts at `at`: a tap, if it is still on it.
    pub(super) fn lift_function(
        &mut self,
        layout: &Layout,
        control: &Control,
        at: [f64; 2],
        now: Instant,
        out: &mut Vec<Input>,
    ) {
        if !contains(layout.control(control.place), [at[0] as f32, at[1] as f32]) {
            return;
        }
        // Whatever the other fingers do ends as the panel opens or closes.
        match control.tap {
            Tap::Panel => {
                self.panel = true;
                self.spend_fingers(now, out);
            }
            Tap::Press(function) => {
                if std::mem::take(&mut self.panel) {
                    self.spend_fingers(now, out);
                }
                out.push(Input::Function(function));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::keys::MIN_STATE;
    use super::super::layout::Rect;
    use super::super::{Cell, Context, HOLD, Here, Phase, TouchEvent};
    use super::*;

    fn playing() -> (Touch, Layout, Instant) {
        let mut touch = Touch::default();
        let now = Instant::now();
        let context = Context {
            mode: Mode::Playing,
            hud_visible: true,
            ..Context::default()
        };
        assert!(touch.frame(context, now).is_empty());
        (touch, Layout::new(2622, 1206, 3.0), now)
    }

    fn finger(
        touch: &mut Touch,
        layout: &Layout,
        id: u64,
        phase: Phase,
        at: [f64; 2],
    ) -> Vec<Input> {
        finger_at(touch, layout, id, phase, at, Instant::now())
    }

    fn finger_at(
        touch: &mut Touch,
        layout: &Layout,
        id: u64,
        phase: Phase,
        at: [f64; 2],
        now: Instant,
    ) -> Vec<Input> {
        let event = TouchEvent {
            id,
            phase,
            at,
            here: Here::default(),
        };
        touch.touch(layout, touch.context, event, now)
    }

    fn centre(r: Rect) -> [f64; 2] {
        [f64::from(r[0] + r[2] / 2.0), f64::from(r[1] + r[3] / 2.0)]
    }

    fn on(layout: &Layout, keys: &str) -> [f64; 2] {
        let control = CONTROLS.iter().find(|c| c.keys == keys).unwrap();
        centre(layout.control(control.place))
    }

    fn tap(touch: &mut Touch, layout: &Layout, at: [f64; 2]) -> Vec<Input> {
        let mut out = finger(touch, layout, 9, Phase::Start, at);
        out.extend(finger(touch, layout, 9, Phase::End, at));
        out
    }

    fn open_panel(touch: &mut Touch, layout: &Layout) {
        tap(touch, layout, on(layout, "Fn"));
        assert!(touch.panel);
    }

    const VIEW: [f64; 2] = [2000.0, 500.0];

    #[test]
    fn a_key_cap_taps_its_key_once_when_lifted_on_it() {
        let (mut touch, layout, _) = playing();
        for (keys, function) in [("F5", Function::Perspective), ("F3", Function::DebugScreen)] {
            let at = on(&layout, keys);
            assert!(finger(&mut touch, &layout, 1, Phase::Start, at).is_empty());
            assert!(finger(&mut touch, &layout, 1, Phase::Move, [at[0] + 5.0, at[1]]).is_empty());
            assert_eq!(
                finger(&mut touch, &layout, 1, Phase::End, [at[0] + 5.0, at[1]]),
                [Input::Function(function)]
            );
        }
    }

    #[test]
    fn a_key_cap_cancelled_or_slid_off_taps_nothing() {
        let (mut touch, layout, _) = playing();
        let f5 = on(&layout, "F5");
        finger(&mut touch, &layout, 1, Phase::Start, f5);
        assert!(finger(&mut touch, &layout, 1, Phase::Cancel, f5).is_empty());
        finger(&mut touch, &layout, 2, Phase::Start, f5);
        assert!(finger(&mut touch, &layout, 2, Phase::Move, VIEW).is_empty());
        assert!(finger(&mut touch, &layout, 2, Phase::End, VIEW).is_empty());
        let fn_cap = on(&layout, "Fn");
        finger(&mut touch, &layout, 3, Phase::Start, fn_cap);
        assert!(finger(&mut touch, &layout, 3, Phase::End, VIEW).is_empty());
        assert!(!touch.panel);
    }

    #[test]
    fn a_tile_taps_its_keys_once_and_closes_the_panel() {
        for control in CONTROLS
            .iter()
            .filter(|c| matches!(c.place, Place::Tile(_)))
        {
            let (mut touch, layout, _) = playing();
            open_panel(&mut touch, &layout);
            let at = centre(layout.control(control.place));
            let Tap::Press(function) = control.tap else {
                panic!("{} opens the panel", control.keys);
            };
            assert_eq!(tap(&mut touch, &layout, at), [Input::Function(function)]);
            assert!(!touch.panel, "{}", control.keys);
            // The key caps are back, and the tile's place is the view again.
            assert_eq!(touch.functions(&layout).len(), 3);
            assert_eq!(tap(&mut touch, &layout, VIEW), [Input::Tap]);
        }
    }

    #[test]
    fn a_tile_cancelled_or_slid_off_taps_nothing_and_the_panel_stays() {
        let (mut touch, layout, _) = playing();
        open_panel(&mut touch, &layout);
        let f1 = on(&layout, "F1");
        finger(&mut touch, &layout, 1, Phase::Start, f1);
        assert!(finger(&mut touch, &layout, 1, Phase::Cancel, f1).is_empty());
        finger(&mut touch, &layout, 2, Phase::Start, f1);
        let beside = on(&layout, "F2");
        assert!(finger(&mut touch, &layout, 2, Phase::End, beside).is_empty());
        assert!(touch.panel);
    }

    #[test]
    fn back_or_a_tap_beyond_the_panel_closes_it_and_does_nothing_else() {
        let (mut touch, layout, _) = playing();
        let back = centre(layout.button(super::super::Button::Back));
        let corner = [
            layout.panel()[0] as f64 - 10.0,
            layout.panel()[1] as f64 - 10.0,
        ];
        for at in [back, VIEW, corner, on(&layout, "F5")] {
            open_panel(&mut touch, &layout);
            assert!(tap(&mut touch, &layout, at).is_empty(), "{at:?}");
            assert!(!touch.panel, "{at:?}");
        }
        // A tap on the panel between its tiles keeps it open.
        open_panel(&mut touch, &layout);
        let [x, y, ..] = layout.panel();
        assert!(
            tap(
                &mut touch,
                &layout,
                [f64::from(x) + 2.0, f64::from(y) + 2.0]
            )
            .is_empty()
        );
        assert!(touch.panel);
    }

    #[test]
    fn opening_the_panel_lets_go_of_what_other_fingers_hold() {
        let (mut touch, layout, now) = playing();
        let ms = |n| now + std::time::Duration::from_millis(n);
        let forward = centre(layout.cell(Cell::Forward));
        finger_at(&mut touch, &layout, 1, Phase::Start, forward, now);
        finger_at(&mut touch, &layout, 2, Phase::Start, VIEW, now);
        let context = touch.context;
        assert_eq!(
            touch.frame(context, now + HOLD),
            [Input::Mapping("key.attack", true)]
        );
        let fn_cap = on(&layout, "Fn");
        finger_at(&mut touch, &layout, 3, Phase::Start, fn_cap, ms(400));
        assert_eq!(
            finger_at(&mut touch, &layout, 3, Phase::End, fn_cap, ms(450)),
            [Input::Mapping("key.attack", false)]
        );
        assert_eq!(touch.move_vector(), None);
        // The old fingers do nothing more, even once the panel has closed.
        tap(&mut touch, &layout, VIEW);
        assert!(!touch.panel);
        assert!(finger_at(&mut touch, &layout, 1, Phase::Move, VIEW, ms(500)).is_empty());
        assert!(finger_at(&mut touch, &layout, 2, Phase::End, VIEW, ms(500)).is_empty());
        assert!(touch.frame(context, ms(500) + MIN_STATE).is_empty());
    }

    #[test]
    fn a_new_screen_closes_the_panel_and_spends_its_fingers() {
        let (mut touch, layout, _) = playing();
        open_panel(&mut touch, &layout);
        let f2 = on(&layout, "F2");
        finger(&mut touch, &layout, 1, Phase::Start, f2);
        let paused = Context {
            mode: Mode::Menu,
            scene: 1,
            ..touch.context
        };
        touch.frame(paused, Instant::now());
        assert!(touch.functions(&layout).is_empty());
        assert!(finger(&mut touch, &layout, 1, Phase::End, f2).is_empty());
        let playing = Context {
            mode: Mode::Playing,
            scene: 2,
            ..touch.context
        };
        touch.frame(playing, Instant::now());
        assert!(!touch.panel);
        let keys: Vec<_> = touch.functions(&layout).iter().map(|s| s.keys).collect();
        assert_eq!(keys, ["F5", "F3", "Fn"]);
    }

    #[test]
    fn a_tap_on_a_shown_control_reaches_it() {
        let (mut touch, layout, _) = playing();
        let shown = touch.functions(&layout);
        let keys: Vec<_> = shown.iter().map(|s| s.keys).collect();
        assert_eq!(keys, ["F5", "F3", "Fn"]);
        assert_eq!(
            tap(&mut touch, &layout, centre(shown[1].rect)),
            [Input::Function(Function::DebugScreen)]
        );
        assert!(tap(&mut touch, &layout, centre(shown[2].rect)).is_empty());
        let shown = touch.functions(&layout);
        let tiles: Vec<_> = shown
            .iter()
            .map(|s| format!("{} {}", s.keys, s.does))
            .collect();
        assert_eq!(
            tiles,
            [
                "F1 Hide HUD",
                "F2 Screenshot",
                "F3+B Hitboxes",
                "F3+G Chunk borders",
                "F3+H Advanced tooltips",
                "F3+F4 Game mode"
            ]
        );
        assert_eq!(
            tap(&mut touch, &layout, centre(shown[5].rect)),
            [Input::Function(Function::GameModes)]
        );
    }
}
