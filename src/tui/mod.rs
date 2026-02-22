mod actions;
mod input;
mod messages;
mod render;

use crate::models::Notification;
use crate::{DbConnectionManager, Pool, get_connection_pool, service};
use anyhow::Result;
use log::{debug, error, info};
use ratatui::DefaultTerminal;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::ListState;
use tokio::sync::mpsc;
use tui_textarea::TextArea;

use messages::*;

pub async fn run() -> Result<()> {
    let res = App::default().run().await;
    ratatui::restore();
    res
}

#[derive(Default)]
struct App<'a> {
    state: UiState,
    popup: Option<Popup>,
    search_text: TextArea<'a>,
    input_mode: InputMode,
    notifications: Vec<Notification>,
    dirty: bool,
    empty_art_index: usize,
    empty_tagline_index: usize,
}

impl App<'_> {
    async fn run(&mut self) -> Result<()> {
        let mut terminal = ratatui::init();
        let mut list_state = ListState::default();
        list_state.select_first();

        let (tx, mut rx) = mpsc::channel::<Message>(32);
        let pool = get_connection_pool();

        self.notifications = actions::refresh(&mut pool.clone().get()?, "").await?;
        self.dirty = false;
        self.reset_search();
        if self.notifications.is_empty() {
            self.randomize_empty_state();
        }
        self.update_ui(
            MessageUi::UiUpdate(UiState::default()),
            tx.clone(),
            &mut terminal,
            &mut list_state,
        )
        .await?;

        let tx_cloned = tx.clone();
        let notif_handle = tokio::spawn(auto_sync_notifs_loop(tx.clone(), pool.clone()));
        let refresh_handle = tokio::spawn(auto_refresh_ui_loop(tx.clone()));
        std::thread::spawn(|| input::handle_input_loop(tx_cloned));

        loop {
            let maybe_message = rx.recv().await;
            if let Some(message) = maybe_message {
                // Determine if this message requires a DB refresh
                let needs_refresh = !matches!(
                    message,
                    Message::Ui(
                        MessageUi::MoveUp(_)
                            | MessageUi::MoveDown(_)
                            | MessageUi::MoveTo(_)
                            | MessageUi::Popup(_)
                    ) | Message::Noop
                );

                if needs_refresh {
                    self.dirty = true;
                }

                if self.dirty {
                    let was_empty = self.notifications.is_empty();
                    self.notifications =
                        actions::refresh(&mut pool.clone().get()?, &self.search_string()).await?;
                    self.dirty = false;
                    if !was_empty && self.notifications.is_empty() {
                        self.randomize_empty_state();
                    }
                }

                if self.popup.is_some() {
                    self.popup = None;
                    self.update_ui(
                        MessageUi::Redraw,
                        tx.clone(),
                        &mut terminal,
                        &mut list_state,
                    )
                    .await?;
                    continue;
                }

                match message {
                    Message::Action(action) => {
                        if action == MessageAction::Quit {
                            break;
                        }
                        let message_action = action.clone();
                        tokio::spawn(actions::handle_action(
                            tx.clone(),
                            pool.clone().get()?,
                            message_action,
                            list_state.selected(),
                            self.notifications.clone(),
                            self.search_string(),
                        ));
                    }
                    Message::Ui(ui) => {
                        self.update_ui(ui, tx.clone(), &mut terminal, &mut list_state)
                            .await?;
                    }
                    Message::Noop => {}
                }
            }
        }

        notif_handle.abort();
        refresh_handle.abort();

        Ok(())
    }

    async fn update_ui(
        &mut self,
        message: MessageUi,
        tx: mpsc::Sender<Message>,
        terminal: &mut DefaultTerminal,
        list_state: &mut ListState,
    ) -> Result<()> {
        match message {
            MessageUi::MoveUp(mov) => {
                list_state.scroll_up_by(mov);
                self.state.reset();
            }
            MessageUi::MoveDown(mov) => {
                list_state.scroll_down_by(mov);
                self.state.reset();
            }
            MessageUi::MoveTo(pos) => {
                list_state.select(Some(pos));
                self.state.reset();
            }
            MessageUi::Redraw => self.state.reset(),
            MessageUi::Popup(popup) => {
                self.popup = Some(popup);
                self.state.reset()
            }
            MessageUi::UiUpdate(update) => {
                self.state = update;
            }
            MessageUi::SearchActivate => {
                self.state.reset();
                self.input_mode = InputMode::Search;
                self.search_text
                    .set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
            }
            MessageUi::SearchInput(c) => {
                self.search_text.input(c);
                self.state.reset();
                tx.send(Message::Ui(MessageUi::Redraw))
                    .await
                    .expect("cannot send");
            }
            MessageUi::SearchQuit => {
                self.reset_search();
                self.state.reset();
                self.input_mode = InputMode::Normal;
            }
            MessageUi::SearchValidated => {
                self.search_text
                    .set_cursor_style(Style::default().add_modifier(Modifier::HIDDEN));
            }
        };

        let headline = if !self.state.error.is_empty() {
            Err(self.state.error.clone())
        } else {
            let info = if self.state.info.is_empty() {
                format!("Riirview, {} notifs", self.notifications.len())
            } else {
                self.state.info.clone()
            };
            if self.state.loading {
                Ok(format!("{} ⌛", info))
            } else {
                Ok(info)
            }
        };

        terminal.draw(|frame| {
            render::draw_frame(
                self,
                frame,
                &self.notifications,
                list_state,
                &self.search_text,
                headline,
            )
        })?;
        Ok(())
    }

    fn search_string(&self) -> String {
        self.search_text.lines()[0].clone()
    }

    fn reset_search(&mut self) {
        self.search_text = TextArea::default();
        let style = Style::default().remove_modifier(Modifier::UNDERLINED);
        self.search_text.set_cursor_line_style(style);
    }

    fn randomize_empty_state(&mut self) {
        use rand::RngExt;
        let mut rng = rand::rng();
        let art_count = render::empty_state_art_count();
        let tagline_count = render::empty_state_tagline_count();
        if art_count > 0 {
            self.empty_art_index = rng.random_range(0..art_count);
        }
        if tagline_count > 0 {
            self.empty_tagline_index = rng.random_range(0..tagline_count);
        }
    }
}

async fn auto_sync_notifs_loop(tx: mpsc::Sender<Message>, pool: Pool<DbConnectionManager>) {
    loop {
        debug!("refreshing notifications");
        let (refresh_delay, need_update) = match pool.get() {
            Err(e) => {
                error!("failed to get DB connection: {}", e);
                (REFRESH_DELAY_SEC, false)
            }
            Ok(conn) => match service::check_update_and_limit(conn).await {
                Err(_) => (REFRESH_DELAY_SEC, true),
                Ok(update_status) => {
                    debug!("gh status {update_status:?}");
                    (
                        std::cmp::max(REFRESH_DELAY_SEC, update_status.poll_interval),
                        update_status.need_update,
                    )
                }
            },
        };

        info!("need_update: {need_update}, sleeping for: {refresh_delay} sec");
        if need_update {
            tx.send(Message::Action(MessageAction::SyncBackground))
                .await
                .expect("cannot send");
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(refresh_delay)).await;
    }
}

async fn auto_refresh_ui_loop(tx: mpsc::Sender<Message>) {
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(REDRAW_DELAY_SEC)).await;
        tx.send(Message::Ui(MessageUi::Redraw))
            .await
            .expect("cannot send");
    }
}

#[cfg(test)]
mod tests {
    use super::messages::KEYMAP;

    #[test]
    fn test_keymap() {
        // This test can be flaky...
        assert!(KEYMAP.contains("up/down           | move cursor up or down"));
        assert!(KEYMAP.split("\n").collect::<Vec<&str>>().len() >= 10);
    }
}
