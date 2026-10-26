//! Selection and hit testing for the debug game-mode switcher.
use crate::command::args::GameMode;

pub const MODES: [GameMode; 4] = [
    GameMode::Creative,
    GameMode::Survival,
    GameMode::Adventure,
    GameMode::Spectator,
];

pub const ICONS: [&str; 4] = [
    "minecraft:grass_block",
    "minecraft:iron_sword",
    "minecraft:buried_treasure_map",
    "minecraft:ender_eye",
];

pub const SLOT_SIZE: i32 = 26;

pub fn slot_origin(width: i32, height: i32, index: usize) -> (i32, i32) {
    (width / 2 - 59 + index as i32 * 31, height / 2 - 31)
}

/// The mode whose slot is under `cursor` (GUI pixels), on a screen `width` x `height`.
pub fn mode_at(width: i32, height: i32, cursor: (i32, i32)) -> Option<GameMode> {
    MODES.into_iter().enumerate().find_map(|(index, mode)| {
        let (x, y) = slot_origin(width, height, index);
        let inside =
            (x..x + SLOT_SIZE).contains(&cursor.0) && (y..y + SLOT_SIZE).contains(&cursor.1);
        inside.then_some(mode)
    })
}

pub struct GameModeSwitcher {
    pub selected: GameMode,
    first_mouse: Option<(i32, i32)>,
}

impl GameModeSwitcher {
    pub fn new(current: GameMode, previous: Option<GameMode>) -> Self {
        Self {
            selected: previous.unwrap_or(if current == GameMode::Creative {
                GameMode::Survival
            } else {
                GameMode::Creative
            }),
            first_mouse: None,
        }
    }

    pub fn cycle(&mut self) {
        let index = MODES.iter().position(|m| *m == self.selected).unwrap();
        self.selected = MODES[(index + 1) % MODES.len()];
        self.first_mouse = None;
    }

    pub fn hover(&mut self, width: i32, height: i32, cursor: (i32, i32)) {
        if *self.first_mouse.get_or_insert(cursor) == cursor {
            return;
        }
        if let Some(mode) = mode_at(width, height, cursor) {
            self.selected = mode;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_selection_and_cycle_follow_debug_order() {
        assert_eq!(
            GameModeSwitcher::new(GameMode::Creative, None).selected,
            GameMode::Survival
        );
        for current in [GameMode::Survival, GameMode::Adventure, GameMode::Spectator] {
            assert_eq!(
                GameModeSwitcher::new(current, None).selected,
                GameMode::Creative
            );
        }
        let mut state = GameModeSwitcher::new(GameMode::Survival, Some(GameMode::Adventure));
        for mode in [
            GameMode::Adventure,
            GameMode::Spectator,
            GameMode::Creative,
            GameMode::Survival,
        ] {
            assert_eq!(state.selected, mode);
            state.cycle();
        }
    }

    #[test]
    fn stationary_mouse_does_not_override_keyboard_selection() {
        let mut state = GameModeSwitcher::new(GameMode::Creative, None);
        let cursor = slot_origin(320, 240, 0);
        state.hover(320, 240, cursor);
        state.hover(320, 240, cursor);
        assert_eq!(state.selected, GameMode::Survival);
        state.hover(320, 240, (cursor.0 + 1, cursor.1));
        assert_eq!(state.selected, GameMode::Creative);
        state.cycle();
        state.hover(320, 240, (cursor.0 + 1, cursor.1));
        assert_eq!(state.selected, GameMode::Survival);
    }

    #[test]
    fn hover_slots_exclude_gaps_and_right_and_bottom_edges() {
        let mut state = GameModeSwitcher::new(GameMode::Creative, None);
        state.hover(320, 240, (0, 0));
        let (x, y) = slot_origin(320, 240, 2);
        for cursor in [(x - 1, y), (x + 26, y), (x, y - 1), (x, y + 26)] {
            state.hover(320, 240, cursor);
            assert_eq!(state.selected, GameMode::Survival);
        }
        state.hover(320, 240, (x + 25, y + 25));
        assert_eq!(state.selected, GameMode::Adventure);
    }

    #[test]
    fn a_point_finds_the_mode_whose_slot_it_is_on() {
        // The Duo unfolded: GUI 357 x 251.
        let (w, h) = (357, 251);
        for (index, mode) in MODES.into_iter().enumerate() {
            let (x, y) = slot_origin(w, h, index);
            for corner in [
                (x, y),
                (x + 25, y),
                (x, y + 25),
                (x + 25, y + 25),
                (x + 13, y + 13),
            ] {
                assert_eq!(mode_at(w, h, corner), Some(mode), "{mode:?} at {corner:?}");
            }
            for edge in [
                (x - 1, y),
                (x + 26, y + 13),
                (x + 13, y - 1),
                (x + 13, y + 26),
            ] {
                assert_ne!(mode_at(w, h, edge), Some(mode), "{mode:?} at {edge:?}");
            }
        }
        let (x, y) = slot_origin(w, h, 1);
        // The 5-pixel gaps between slots, and the panel's title and help lines.
        assert_eq!(mode_at(w, h, (x - 3, y + 13)), None);
        assert_eq!(mode_at(w, h, (x + 13, h / 2 - 51)), None);
        assert_eq!(mode_at(w, h, (x + 13, h / 2 + 5)), None);
    }
}
