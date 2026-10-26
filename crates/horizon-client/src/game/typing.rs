//! The text field that has the keyboard's focus, and the one way text reaches it: from the pad's
//! on-screen keyboard, `horizon-ctl type` and, on iOS, the system keyboard, which this module
//! also shows and hides.
use super::{Game, Loading, Screen, books::BookScreen};
use serde_json::{Value, json};
use winit::{
    event::{Ime, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

/// A text field with the keyboard's focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Field {
    Chat,
    Sign,
    /// A book and quill's page, or its title while signing.
    Book,
    CommandBlock,
    CreativeSearch,
    RecipeSearch,
    /// The anvil's name.
    Anvil,
    /// The LAN port, or the game rules' search and number boxes.
    WorldOptions,
    /// The language, settings and pack search boxes.
    Options,
    /// The title screens' fields: world names and seeds, the world search, server addresses.
    Title,
}

impl Field {
    /// A screen made for typing: the system keyboard opens with it. Elsewhere it waits for a tap
    /// on the field, so it doesn't cover the slots or the list the player came for.
    fn opens_keyboard(self) -> bool {
        matches!(
            self,
            Field::Chat | Field::Sign | Field::Book | Field::CommandBlock
        )
    }
    /// Enter ends the typing here; on a sign or a book's page it starts the next line.
    fn enter_ends(self) -> bool {
        !matches!(self, Field::Sign | Field::Book)
    }
}

/// What typing does to the focused field, as the key that makes it would.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Edit {
    Text(String),
    Backspace,
    Enter,
}

/// What the system keyboard tells the window, which the screens' handlers don't know.
pub(super) enum SystemInput<'a> {
    /// Typed text (`insertText:`), as a key no keyboard has: the text of its press, none for its
    /// release.
    Typed(Option<&'a str>),
    /// The window took the keyboard or gave it up (`Ime::Enabled`, `Ime::Disabled`), whoever
    /// asked: the iPad's hide key gives it up.
    Shown(bool),
}

/// The system keyboard's input among the window's events. Only iOS has one.
pub(super) fn system_input(event: &WindowEvent) -> Option<SystemInput<'_>> {
    if !cfg!(target_os = "ios") {
        return None;
    }
    match event {
        WindowEvent::KeyboardInput { event, .. }
            if matches!(event.physical_key, PhysicalKey::Unidentified(_)) =>
        {
            Some(SystemInput::Typed(event.text.as_deref()))
        }
        WindowEvent::Ime(Ime::Enabled) => Some(SystemInput::Shown(true)),
        WindowEvent::Ime(Ime::Disabled) => Some(SystemInput::Shown(false)),
        _ => None,
    }
}

/// The system keyboard: whether the player wants it and whether the window has it, kept to the
/// focused field.
#[derive(Debug, Default)]
pub(super) struct SystemKeyboard {
    wanted: bool,
    /// The window has the keyboard, as iOS reports it. A hardware keyboard keeps the on-screen
    /// one away meanwhile.
    shown: bool,
    /// The focused field at the last step: a change decides `wanted` again.
    field: Option<Field>,
}

impl SystemKeyboard {
    /// The step for the field focused now: `Some(show)` when the keyboard must appear or go.
    /// Repeated with the same field, it changes nothing.
    fn step(&mut self, field: Option<Field>) -> Option<bool> {
        if field != self.field {
            self.field = field;
            self.wanted = field.is_some_and(Field::opens_keyboard);
        }
        (self.wanted != self.shown).then(|| {
            self.shown = self.wanted;
            self.shown
        })
    }
    /// A tap left `field` with the focus: the keyboard is wanted while it keeps it, and asked
    /// for even if the window has it, since a hardware keyboard may have put it away.
    fn request(&mut self, field: Option<Field>) -> Option<bool> {
        self.field = field;
        self.wanted = field.is_some();
        if !self.wanted {
            return self.step(field);
        }
        self.shown = true;
        Some(true)
    }
    /// What the system keyboard sent, with `field` focused: the edit it makes there.
    fn input(&mut self, field: Option<Field>, input: SystemInput) -> Option<(Field, Edit)> {
        let text = match input {
            SystemInput::Typed(text) => text?,
            SystemInput::Shown(shown) => {
                // Put away by the player, it stays away until the next tap.
                if !shown {
                    self.wanted = false;
                }
                self.shown = shown;
                return None;
            }
        };
        let field = field?;
        let edit = match text {
            "\n" | "\r" => Edit::Enter,
            text => Edit::Text(text.to_owned()),
        };
        if edit == Edit::Enter && field.enter_ends() {
            self.wanted = false;
        }
        Some((field, edit))
    }
}

/// Shows or hides the system keyboard. Only iOS has one: elsewhere `SystemKeyboard` says what iOS
/// would show.
fn show_keyboard(window: Option<&Window>, show: Option<bool>) {
    if let Some(show) = show
        && cfg!(target_os = "ios")
        && let Some(window) = window
    {
        window.set_ime_allowed(show);
    }
}

impl Game<'_> {
    /// The field of the screen that takes the keys, in the order `window_event` gives them to
    /// the screens. The options', World Options' and the title's may not have the focus.
    pub(super) fn front_field(&self) -> Option<Field> {
        if self.pending_entry.is_some()
            || self.world_loader.is_some()
            || self.pending_save.is_some()
            || self.server_packs.prompt.is_some()
        {
            return None;
        }
        if self.screen == Screen::Chat && self.title.is_none() {
            return Some(Field::Chat);
        }
        if self.sign_edit.is_some() {
            return Some(Field::Sign);
        }
        if let Some(book) = &self.book {
            return matches!(book, BookScreen::Edit { .. }).then_some(Field::Book);
        }
        if self.screen == Screen::Creative && self.creative_screen.tab().search {
            return Some(Field::CreativeSearch);
        }
        if self.recipe_search_focused() {
            return Some(Field::RecipeSearch);
        }
        if self.station_typing() {
            return Some(Field::Anvil);
        }
        match self.screen {
            Screen::WorldOptions => Some(Field::WorldOptions),
            Screen::Win => None,
            Screen::CommandBlock => Some(Field::CommandBlock),
            Screen::Options => Some(Field::Options),
            _ => self.title.is_some().then_some(Field::Title),
        }
    }

    /// The focused field.
    pub(super) fn typing_field(&self) -> Option<Field> {
        let field = self.front_field()?;
        let focused = match field {
            Field::WorldOptions => self.world_options_typing(),
            Field::Options => self.ui.options.typing(),
            Field::Title => self.title.as_ref().is_some_and(|t| t.typing()),
            _ => true,
        };
        focused.then_some(field)
    }

    /// An edit typed into the focused field. False with no field.
    pub(super) fn type_into_field(&mut self, edit: Edit) -> bool {
        let Some(field) = self.typing_field() else {
            return false;
        };
        self.edit_field(field, edit);
        true
    }

    /// An edit typed into `field`, as the key that makes it would.
    pub(super) fn edit_field(&mut self, field: Field, edit: Edit) {
        let (key, text) = match &edit {
            Edit::Text(text) => (None, Some(text.as_str())),
            Edit::Backspace => (Some(KeyCode::Backspace), None),
            Edit::Enter => (Some(KeyCode::Enter), None),
        };
        let backspace = edit == Edit::Backspace;
        match field {
            Field::Chat => match &edit {
                Edit::Text(text) => self.chat_type(text),
                Edit::Backspace => self.chat_key_code(KeyCode::Backspace, None),
                Edit::Enter => self.chat_key_code(KeyCode::Enter, None),
            },
            Field::Sign => self.sign_input(key, text),
            Field::Book => self.book_input(key, text),
            Field::CommandBlock => match &edit {
                Edit::Text(text) => self.command_block_type(text),
                Edit::Backspace => self.command_block_backspace(),
                Edit::Enter => self.command_block_done(),
            },
            // Enter leaves the search boxes, the anvil's name and World Options' boxes as they are.
            Field::CreativeSearch | Field::RecipeSearch | Field::Anvil | Field::WorldOptions
                if edit == Edit::Enter => {}
            Field::CreativeSearch => self.creative_search_edit(text, backspace),
            Field::RecipeSearch => self.recipe_text(text, backspace),
            Field::Anvil => self.station_text(text, backspace),
            Field::WorldOptions => self.world_options_text(text),
            Field::Options => {
                let result = match &edit {
                    Edit::Text(text) => {
                        self.ui.options.type_text(&self.gui.atlas, text);
                        Ok(())
                    }
                    Edit::Backspace => self.options_key(KeyCode::Backspace, None),
                    Edit::Enter => self.options_key(KeyCode::Enter, None),
                };
                if let Err(e) = result {
                    log::warn!("Horizon: typing in the options: {e}");
                }
            }
            Field::Title => match &edit {
                Edit::Enter => {
                    if let Some(title) = self.title.as_mut() {
                        title.confirm_edit();
                    }
                }
                _ => self.title_edit(text, backspace),
            },
        }
    }

    /// Once a frame: the system keyboard follows the focused field.
    pub(super) fn system_keyboard_frame(&mut self) {
        let show = self.keyboard.step(self.typing_field());
        show_keyboard(self.window.as_deref(), show);
    }

    /// A tap on the focused field: the system keyboard comes up for it.
    pub(super) fn request_keyboard(&mut self) {
        let show = self.keyboard.request(self.typing_field());
        show_keyboard(self.window.as_deref(), show);
    }

    /// The system keyboard's input, before any screen's handler.
    pub(super) fn system_keyboard_input(&mut self, input: SystemInput) {
        if let Some((field, edit)) = self.keyboard.input(self.typing_field(), input) {
            self.edit_field(field, edit);
        }
    }

    /// `horizon-ctl keyboard`: on other platforms than iOS, `shown` is what iOS would show.
    pub(super) fn keyboard_status(&self) -> Value {
        json!({
            "field": self.typing_field().map(|f| format!("{f:?}")),
            "wanted": self.keyboard.wanted,
            "shown": self.keyboard.shown,
        })
    }
}

impl Loading {
    /// Once a frame: the system keyboard follows the startup menu's focused field.
    pub(super) fn system_keyboard_frame(&mut self) {
        let show = self.keyboard.step(self.typing_field());
        show_keyboard(self.window.as_deref(), show);
    }

    /// A tap on a title or options field: the system keyboard comes up for it.
    #[expect(
        dead_code,
        reason = "the touch layer calls it once its field taps are wired"
    )]
    pub(super) fn request_keyboard(&mut self) {
        let show = self.keyboard.request(self.typing_field());
        show_keyboard(self.window.as_deref(), show);
    }

    pub(super) fn system_keyboard_input(&mut self, input: SystemInput) {
        if let Some((field, edit)) = self.keyboard.input(self.typing_field(), input) {
            self.edit_field(field, edit);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_typing_screen_shows_the_keyboard_once_and_its_closing_hides_it() {
        let mut keyboard = SystemKeyboard::default();
        assert_eq!(keyboard.step(None), None);
        assert_eq!(keyboard.step(Some(Field::Chat)), Some(true));
        assert_eq!(keyboard.step(Some(Field::Chat)), None);
        assert_eq!(keyboard.step(None), Some(false));
        assert_eq!(keyboard.step(None), None);
        // The window reports what it was asked: nothing changes.
        assert_eq!(keyboard.input(None, SystemInput::Shown(false)), None);
        assert_eq!(keyboard.step(None), None);
        assert_eq!(keyboard.step(Some(Field::Chat)), Some(true));
        assert_eq!(
            keyboard.input(Some(Field::Chat), SystemInput::Shown(true)),
            None
        );
        assert_eq!(keyboard.step(Some(Field::Chat)), None);
    }

    #[test]
    fn a_field_focused_without_a_tap_waits_for_one() {
        let mut keyboard = SystemKeyboard::default();
        assert_eq!(keyboard.step(Some(Field::CreativeSearch)), None);
        assert_eq!(keyboard.request(Some(Field::CreativeSearch)), Some(true));
        assert_eq!(keyboard.step(Some(Field::CreativeSearch)), None);
        // Another tab: the search box loses the focus.
        assert_eq!(keyboard.step(None), Some(false));
        assert_eq!(keyboard.step(Some(Field::CreativeSearch)), None);
    }

    #[test]
    fn a_tap_with_no_field_focused_asks_for_nothing() {
        let mut keyboard = SystemKeyboard::default();
        assert_eq!(keyboard.request(None), None);
        assert_eq!(keyboard.step(None), None);
    }

    #[test]
    fn a_keyboard_the_player_put_away_stays_away_until_a_tap_brings_it_back() {
        let mut keyboard = SystemKeyboard::default();
        assert_eq!(keyboard.step(Some(Field::Chat)), Some(true));
        // The iPad's hide key: the window gives the keyboard up.
        assert_eq!(
            keyboard.input(Some(Field::Chat), SystemInput::Shown(false)),
            None
        );
        assert_eq!(keyboard.step(Some(Field::Chat)), None);
        assert_eq!(keyboard.request(Some(Field::Chat)), Some(true));
        assert_eq!(
            keyboard.input(Some(Field::Chat), SystemInput::Shown(true)),
            None
        );
        assert_eq!(keyboard.step(Some(Field::Chat)), None);
        // A hardware keyboard put it away without the window giving it up: a tap asks again.
        assert_eq!(keyboard.request(Some(Field::Chat)), Some(true));
        assert_eq!(keyboard.step(Some(Field::Chat)), None);
    }

    #[test]
    fn return_ends_the_typing_in_a_one_line_field_but_starts_a_line_on_a_sign() {
        let mut keyboard = SystemKeyboard::default();
        let typed = |keyboard: &mut SystemKeyboard, field, text| {
            keyboard.input(Some(field), SystemInput::Typed(Some(text)))
        };
        assert_eq!(keyboard.request(Some(Field::Anvil)), Some(true));
        assert_eq!(
            typed(&mut keyboard, Field::Anvil, "é"),
            Some((Field::Anvil, Edit::Text("é".into())))
        );
        assert_eq!(keyboard.step(Some(Field::Anvil)), None);
        assert_eq!(
            typed(&mut keyboard, Field::Anvil, "\n"),
            Some((Field::Anvil, Edit::Enter))
        );
        assert_eq!(keyboard.step(Some(Field::Anvil)), Some(false));
        // The name keeps the focus: the keyboard stays away until the next tap.
        assert_eq!(keyboard.step(Some(Field::Anvil)), None);

        assert_eq!(keyboard.step(Some(Field::Sign)), Some(true));
        assert_eq!(
            typed(&mut keyboard, Field::Sign, "\r"),
            Some((Field::Sign, Edit::Enter))
        );
        assert_eq!(keyboard.step(Some(Field::Sign)), None);
    }

    #[test]
    fn the_system_keyboard_edits_nothing_without_a_field_or_on_a_release() {
        let mut keyboard = SystemKeyboard::default();
        assert_eq!(keyboard.input(None, SystemInput::Typed(Some("a"))), None);
        assert_eq!(
            keyboard.input(Some(Field::Chat), SystemInput::Typed(None)),
            None
        );
    }

    #[test]
    fn moving_between_fields_decides_again() {
        let mut keyboard = SystemKeyboard::default();
        assert_eq!(keyboard.request(Some(Field::Title)), Some(true));
        // The world opens from the title, and its chat opens.
        assert_eq!(keyboard.step(None), Some(false));
        assert_eq!(keyboard.step(Some(Field::Chat)), Some(true));
        // Chat closes onto the options' search, which waits for a tap.
        assert_eq!(keyboard.step(Some(Field::Options)), Some(false));
    }
}
