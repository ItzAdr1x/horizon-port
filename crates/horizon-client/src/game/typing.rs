//! The text field that has the keyboard's focus, and the one way text reaches it: from the pad's
//! on-screen keyboard, `horizon-ctl type` and, on iOS, the system keyboard, which this module
//! also shows and hides.
use super::{Game, Loading, Screen, books::BookScreen};
use serde_json::{Value, json};
use winit::{keyboard::KeyCode, window::Window};

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

/// The system keyboard: whether the player wants it and whether the window was asked to show
/// it, kept to the focused field. Only iOS has one: elsewhere nothing reaches the window.
#[derive(Debug, Default)]
pub(super) struct SystemKeyboard {
    wanted: bool,
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
    /// A tap left `field` with the focus: the keyboard is wanted while it keeps it.
    fn request(&mut self, field: Option<Field>) {
        self.field = field;
        self.wanted = field.is_some();
    }
    /// Enter was typed into `field`.
    fn entered(&mut self, field: Field) {
        if field.enter_ends() {
            self.wanted = false;
        }
    }
    fn apply(&mut self, field: Option<Field>, window: Option<&Window>) {
        if let Some(show) = self.step(field)
            && cfg!(target_os = "ios")
            && let Some(window) = window
        {
            window.set_ime_allowed(show);
        }
    }
}

/// The edit a piece of the system keyboard's text makes: Return comes as a new line.
fn system_edit(text: &str) -> Edit {
    match text {
        "\n" | "\r" => Edit::Enter,
        text => Edit::Text(text.to_owned()),
    }
}

impl Game<'_> {
    /// The focused field, in the order `window_event` gives keys to the screens.
    pub(super) fn typing_field(&self) -> Option<Field> {
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
            Screen::WorldOptions => self.world_options_typing().then_some(Field::WorldOptions),
            Screen::Win => None,
            Screen::CommandBlock => Some(Field::CommandBlock),
            Screen::Options => self.ui.options.typing().then_some(Field::Options),
            _ => self
                .title
                .as_ref()
                .is_some_and(|t| t.typing())
                .then_some(Field::Title),
        }
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
    fn edit_field(&mut self, field: Field, edit: Edit) {
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
        let field = self.typing_field();
        self.keyboard.apply(field, self.window.as_deref());
    }

    /// A tap gave a text field the focus: the system keyboard comes up for it.
    pub(super) fn request_keyboard(&mut self) {
        let field = self.typing_field();
        self.keyboard.request(field);
        self.system_keyboard_frame();
    }

    /// Text from the system keyboard (`insertText:`), before any screen's key handler. Enter in
    /// a one-line field ends the typing, and the keyboard goes.
    pub(super) fn system_keyboard_text(&mut self, text: &str) {
        let Some(field) = self.typing_field() else {
            return;
        };
        let edit = system_edit(text);
        if edit == Edit::Enter {
            self.keyboard.entered(field);
        }
        self.type_into_field(edit);
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
        let field = self.typing_field();
        self.keyboard.apply(field, self.window.as_deref());
    }

    /// A tap gave a title or options field the focus: the system keyboard comes up for it.
    #[expect(
        dead_code,
        reason = "the touch layer calls it once its field taps are wired"
    )]
    pub(super) fn request_keyboard(&mut self) {
        let field = self.typing_field();
        self.keyboard.request(field);
        self.system_keyboard_frame();
    }

    pub(super) fn system_keyboard_text(&mut self, text: &str) {
        let Some(field) = self.typing_field() else {
            return;
        };
        let edit = system_edit(text);
        if edit == Edit::Enter {
            self.keyboard.entered(field);
        }
        self.type_into_field(edit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn return_from_the_system_keyboard_is_enter_and_the_rest_is_text() {
        assert_eq!(system_edit("\n"), Edit::Enter);
        assert_eq!(system_edit("\r"), Edit::Enter);
        assert_eq!(system_edit("/"), Edit::Text("/".into()));
        assert_eq!(system_edit("é"), Edit::Text("é".into()));
        assert_eq!(system_edit("a\nb"), Edit::Text("a\nb".into()));
    }

    #[test]
    fn only_typing_screens_open_the_keyboard_and_only_signs_and_books_keep_it_on_enter() {
        use Field::*;
        let all = [
            Chat,
            Sign,
            Book,
            CommandBlock,
            CreativeSearch,
            RecipeSearch,
            Anvil,
            WorldOptions,
            Options,
            Title,
        ];
        let opening: Vec<Field> = all.into_iter().filter(|f| f.opens_keyboard()).collect();
        assert_eq!(opening, [Chat, Sign, Book, CommandBlock]);
        let keeping: Vec<Field> = all.into_iter().filter(|f| !f.enter_ends()).collect();
        assert_eq!(keeping, [Sign, Book]);
    }

    #[test]
    fn a_typing_screen_shows_the_keyboard_once_and_its_closing_hides_it() {
        let mut keyboard = SystemKeyboard::default();
        assert_eq!(keyboard.step(None), None);
        assert_eq!(keyboard.step(Some(Field::Chat)), Some(true));
        assert_eq!(keyboard.step(Some(Field::Chat)), None);
        assert_eq!(keyboard.step(None), Some(false));
        assert_eq!(keyboard.step(None), None);
    }

    #[test]
    fn a_field_focused_without_a_tap_waits_for_one() {
        let mut keyboard = SystemKeyboard::default();
        assert_eq!(keyboard.step(Some(Field::CreativeSearch)), None);
        keyboard.request(Some(Field::CreativeSearch));
        assert_eq!(keyboard.step(Some(Field::CreativeSearch)), Some(true));
        assert_eq!(keyboard.step(Some(Field::CreativeSearch)), None);
        // Another tab: the search box loses the focus.
        assert_eq!(keyboard.step(None), Some(false));
        assert_eq!(keyboard.step(Some(Field::CreativeSearch)), None);
    }

    #[test]
    fn a_tap_with_no_field_focused_asks_for_nothing() {
        let mut keyboard = SystemKeyboard::default();
        keyboard.request(None);
        assert_eq!(keyboard.step(None), None);
    }

    #[test]
    fn enter_hides_the_keyboard_of_a_one_line_field_but_not_of_a_sign() {
        let mut keyboard = SystemKeyboard::default();
        keyboard.request(Some(Field::Anvil));
        assert_eq!(keyboard.step(Some(Field::Anvil)), Some(true));
        keyboard.entered(Field::Anvil);
        assert_eq!(keyboard.step(Some(Field::Anvil)), Some(false));
        // The name keeps the focus: the keyboard stays away until the next tap.
        assert_eq!(keyboard.step(Some(Field::Anvil)), None);

        assert_eq!(keyboard.step(Some(Field::Sign)), Some(true));
        keyboard.entered(Field::Sign);
        assert_eq!(keyboard.step(Some(Field::Sign)), None);
    }

    #[test]
    fn moving_between_fields_decides_again() {
        let mut keyboard = SystemKeyboard::default();
        keyboard.request(Some(Field::Title));
        assert_eq!(keyboard.step(Some(Field::Title)), Some(true));
        // The world opens from the title, and its chat opens.
        assert_eq!(keyboard.step(None), Some(false));
        assert_eq!(keyboard.step(Some(Field::Chat)), Some(true));
        // Chat closes onto the options' search, which waits for a tap.
        assert_eq!(keyboard.step(Some(Field::Options)), Some(false));
    }
}
