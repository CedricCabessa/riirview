use crate::tui::messages::{InputMode, Message, MessageAction, MessageUi, UiState};
use ratatui::crossterm::event::{self, Event, KeyCode};
use tokio::sync::mpsc;

pub(crate) fn handle_input_loop(tx: mpsc::Sender<Message>) {
    let mut input_mode = InputMode::Normal;
    loop {
        let event = event::read();
        if let Err(err) = event {
            tx.blocking_send(Message::Ui(MessageUi::UiUpdate(UiState::error_msg(
                err.to_string(),
            ))))
            .expect("cannot send");
            return;
        }

        if let Ok(Event::Key(key)) = event {
            let message = match input_mode {
                InputMode::Normal => {
                    match key.code {
                        KeyCode::Down => Message::Ui(MessageUi::MoveDown(1)),
                        KeyCode::PageDown => Message::Ui(MessageUi::MoveDown(10)),
                        KeyCode::Up => Message::Ui(MessageUi::MoveUp(1)),
                        KeyCode::PageUp => Message::Ui(MessageUi::MoveUp(10)),
                        KeyCode::Char('/') => {
                            input_mode = InputMode::Search;
                            Message::Ui(MessageUi::SearchActivate)
                        }
                        KeyCode::Esc => {
                            //needs 2 messages
                            tx.blocking_send(Message::Ui(MessageUi::SearchQuit))
                                .expect("cannot send message");
                            Message::Ui(MessageUi::Redraw)
                        }
                        KeyCode::Char('q') => Message::Action(MessageAction::Quit),
                        KeyCode::Char('+') => Message::Action(MessageAction::ScoreIncrement(10)),
                        KeyCode::Char('-') => Message::Action(MessageAction::ScoreIncrement(-10)),
                        KeyCode::Enter => Message::Action(MessageAction::Open),
                        KeyCode::Char('r') => Message::Action(MessageAction::MarkAsDone),
                        KeyCode::Char('R') => Message::Action(MessageAction::MarkBelowAsDone),
                        KeyCode::Char('g') => Message::Action(MessageAction::Sync),
                        KeyCode::Char('x') => Message::Action(MessageAction::Explain),
                        KeyCode::Char('?') => Message::Action(MessageAction::Help),
                        _ => Message::Noop,
                    }
                }
                InputMode::Search => {
                    match key.code {
                        KeyCode::Enter => {
                            input_mode = InputMode::Normal;
                            Message::Ui(MessageUi::SearchValidated)
                        }
                        KeyCode::Esc => {
                            input_mode = InputMode::Normal;
                            //needs 2 messages
                            tx.blocking_send(Message::Ui(MessageUi::SearchQuit))
                                .expect("cannot send message");
                            Message::Ui(MessageUi::Redraw)
                        }
                        _ => Message::Ui(MessageUi::SearchInput(Event::Key(key))),
                    }
                }
            };

            // send message, it will be executed if popup is inactive
            tx.blocking_send(message).expect("cannot send message");
        }
    }
}
