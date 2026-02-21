use ratatui::crossterm::event::Event;

#[derive(Clone, PartialEq, Debug)]
pub(crate) enum Message {
    Action(MessageAction),
    Ui(MessageUi),
    Noop,
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) enum MessageAction {
    Quit,
    ScoreIncrement(i32),
    Open,
    MarkAsDone,
    MarkBelowAsDone,
    Sync,
    SyncBackground,
    Explain,
    Help,
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) enum MessageUi {
    MoveUp(u16),
    MoveDown(u16),
    MoveTo(usize),
    UiUpdate(UiState),
    Popup(Popup),
    SearchActivate,
    SearchInput(Event),
    SearchValidated,
    SearchQuit,
    Redraw,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub(crate) struct UiState {
    pub(crate) info: String,
    pub(crate) error: String,
    pub(crate) loading: bool,
}

impl UiState {
    pub(crate) fn reset(&mut self) {
        if !self.loading {
            self.info.clear();
            self.error.clear();
        }
    }

    pub(crate) fn info_msg(message: String) -> Self {
        UiState {
            info: message,
            error: String::new(),
            loading: false,
        }
    }

    pub(crate) fn loading_msg(message: String) -> Self {
        UiState {
            info: message,
            error: String::new(),
            loading: true,
        }
    }

    pub(crate) fn error_msg(message: String) -> Self {
        UiState {
            info: String::new(),
            error: message,
            loading: false,
        }
    }
}

#[derive(Default, Clone, Debug, PartialEq)]
pub(crate) struct Popup {
    pub(crate) title: String,
    pub(crate) content: String,
}

#[derive(Default, Clone, PartialEq)]
pub(crate) enum InputMode {
    #[default]
    Normal,
    Search,
}

pub(crate) const REFRESH_DELAY_SEC: u64 = 300;
pub(crate) const REDRAW_DELAY_SEC: u64 = 60;

// define KEYMAP str constant with key binding info
// The generated file contains `const KEYMAP: &str = ...;` — we re-export it as pub(crate).
mod keymap_generated {
    include!(concat!(env!("OUT_DIR"), "/keymap.rs"));
    pub(crate) const KEYMAP_VALUE: &str = KEYMAP;
}
pub(crate) use keymap_generated::KEYMAP_VALUE as KEYMAP;
