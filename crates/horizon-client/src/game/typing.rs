//! The text field that has the keyboard's focus, and the one way text reaches it: from the pad's
//! on-screen keyboard and `horizon-ctl type`.
use super::{Game, Screen, books::BookScreen};
use horizon_client::osk::Output as Edit;
use winit::keyboard::KeyCode;

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

    /// An edit typed into the focused field, as the key that makes it would. False with no
    /// field. `Close` is the pad keyboard's own: the field stays as it is.
    pub(super) fn type_into_field(&mut self, edit: Edit) -> bool {
        let Some(field) = self.typing_field() else {
            return false;
        };
        let (key, text) = match &edit {
            Edit::Text(text) => (None, Some(text.as_str())),
            Edit::Backspace => (Some(KeyCode::Backspace), None),
            Edit::Enter => (Some(KeyCode::Enter), None),
            Edit::Close => return true,
        };
        let backspace = edit == Edit::Backspace;
        match field {
            Field::Chat => match (key, text) {
                (Some(key), _) => self.chat_key_code(key, None),
                (None, text) => self.chat_type(text.unwrap_or_default()),
            },
            Field::Sign => self.sign_input(key, text),
            Field::Book => self.book_input(key, text),
            Field::CommandBlock => match edit {
                Edit::Enter => self.command_block_done(),
                Edit::Backspace => self.command_block_backspace(),
                _ => self.command_block_type(text.unwrap_or_default()),
            },
            // Enter leaves the search boxes, the anvil's name and World Options' boxes as they are.
            Field::CreativeSearch | Field::RecipeSearch | Field::Anvil | Field::WorldOptions
                if edit == Edit::Enter => {}
            Field::CreativeSearch => self.creative_search_edit(text, backspace),
            Field::RecipeSearch => self.recipe_text(text, backspace),
            Field::Anvil => self.station_text(text, backspace),
            Field::WorldOptions => self.world_options_text(text),
            Field::Options => {
                let result = match (key, text) {
                    (Some(key), _) => self.options_key(key, None),
                    (None, text) => {
                        let text = text.unwrap_or_default();
                        self.ui.options.type_text(&self.gui.atlas, text);
                        Ok(())
                    }
                };
                if let Err(e) = result {
                    log::warn!("Horizon: typing in the options: {e}");
                }
            }
            Field::Title => match edit {
                Edit::Enter => {
                    if let Some(title) = self.title.as_mut() {
                        title.confirm_edit();
                    }
                }
                _ => self.title_edit(text, backspace),
            },
        }
        true
    }
}
