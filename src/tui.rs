use crate::gh::Error as GhError;
use crate::models::{Notification, NotificationState, NotificationType};
use crate::score::Error as ScoreError;
use crate::{DbConnection, DbConnectionManager, Pool, get_connection_pool, service};
use anyhow::Result;
use chrono_humanize::{Accuracy, HumanTime, Tense};
use log::{debug, error, info};
use ratatui::crossterm::event::{self, Event, KeyCode};
use ratatui::style::{Modifier, Style};
use ratatui::text::*;
use ratatui::widgets::{ListState, Paragraph};
use ratatui::{
    DefaultTerminal,
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::Color,
    text::Line,
    widgets::{Block, Clear, List},
};
use tokio::sync::mpsc;

use ratatui::Frame;

#[derive(Clone, PartialEq, Debug)]
enum Message {
    Action(MessageAction),
    Ui(MessageUi),
    Noop,
}

#[derive(Clone, PartialEq, Debug)]
enum MessageAction {
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
enum MessageUi {
    MoveUp(u16),
    MoveDown(u16),
    Error(String),
    Info(String),
    Popup(Popup),
    Redraw,
}

enum Headline {
    Info(String),
    Error(String),
}

impl Default for Headline {
    fn default() -> Self {
        Headline::Info("".into())
    }
}

#[derive(Default, Clone, Debug, PartialEq)]
struct Popup {
    title: String,
    content: String,
}

const REFRESH_DELAY_SEC: u64 = 300;
const REDRAW_DELAY_SEC: u64 = 60;

// define KEYMAP str constant with key binding info
include!(concat!(env!("OUT_DIR"), "/keymap.rs"));

pub async fn run() -> Result<()> {
    let res = App::default().run().await;
    ratatui::restore();
    res
}

#[derive(Default)]
struct App {
    headline: Headline,
    popup: Option<Popup>,
}

impl App {
    async fn run(&mut self) -> Result<()> {
        let mut terminal = ratatui::init();
        let mut list_state = ListState::default();
        list_state.select_first();

        let (tx, mut rx) = mpsc::channel::<Message>(32);
        let pool = get_connection_pool();

        let notifications = refresh(pool.clone().get()?).await?;
        terminal.draw(|frame| self.draw(frame, &notifications, &mut list_state))?;

        let tx_cloned = tx.clone();
        let notif_handle = tokio::spawn(auto_sync_notifs_loop(tx.clone(), pool.clone()));
        let refresh_handle = tokio::spawn(auto_refresh_ui_loop(tx.clone()));
        std::thread::spawn(|| handle_input_loop(tx_cloned));

        loop {
            let maybe_message = rx.recv().await;
            if let Some(message) = maybe_message {
                // FIXME: fetch notif (in db) for *every* ui event (move up/down, etc.)
                // it should be done only after a change in the list
                let notifications = refresh(pool.clone().get()?).await?;

                if self.popup.is_some() {
                    self.popup = None;
                    self.update_ui(
                        MessageUi::Redraw,
                        &mut terminal,
                        &mut list_state,
                        &notifications,
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
                        tokio::spawn(handle_action(
                            tx.clone(),
                            pool.clone().get()?,
                            message_action,
                            list_state.selected(),
                            notifications,
                        ));
                    }
                    Message::Ui(ui) => {
                        self.update_ui(ui, &mut terminal, &mut list_state, &notifications)
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

    fn draw(
        &self,
        frame: &mut Frame,
        notifications: &Vec<Notification>,
        list_state: &mut ListState,
    ) {
        let status = match &self.headline {
            Headline::Info(info) => {
                if info.is_empty() {
                    Span::raw(format!("Riirview, {} notifs", notifications.len()))
                } else {
                    Span::raw(info.to_string())
                }
            }
            Headline::Error(error) => Span::raw(error.clone()).style(Color::Red),
        };

        let head = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]);
        let [status_rect, help_rect] = head.areas(frame.area());
        frame.render_widget(Line::from(status).alignment(Alignment::Left), status_rect);
        frame.render_widget(
            Line::from("? for Help").alignment(Alignment::Right),
            help_rect,
        );

        let layout_v = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).spacing(1);
        let [_, main_area] = layout_v.areas(frame.area());

        let list = List::new(notifications).highlight_style(Modifier::REVERSED);
        frame.render_stateful_widget(list, main_area, list_state);
        if let Some(popup) = &self.popup {
            let area = frame.area();
            let lines: Vec<Line> = popup.content.split('\n').map(Line::from).collect();
            let max_len = lines
                .iter()
                .max_by(|a, b| a.width().cmp(&b.width()))
                .unwrap();
            let height: u16 = (lines.len() + 3).try_into().unwrap();
            let width: u16 = (max_len.width() + 3).try_into().unwrap();
            let area = popup_area(area, height, width);
            let paragraph = Paragraph::new(lines);
            let block = Block::bordered().title(popup.title.as_str());
            frame.render_widget(Clear, area); //this clears out the background
            frame.render_widget(paragraph.block(block), area);
        }
    }

    async fn update_ui(
        &mut self,
        message: MessageUi,
        terminal: &mut DefaultTerminal,
        list_state: &mut ListState,
        notifications: &Vec<Notification>,
    ) -> Result<()> {
        match message {
            MessageUi::MoveUp(mov) => {
                list_state.scroll_up_by(mov);
                self.headline = Headline::default();
            }
            MessageUi::MoveDown(mov) => {
                list_state.scroll_down_by(mov);
                self.headline = Headline::default();
            }
            MessageUi::Error(err) => self.headline = Headline::Error(err),
            MessageUi::Info(info) => self.headline = Headline::Info(info),
            MessageUi::Redraw => self.headline = Headline::default(),
            MessageUi::Popup(popup) => {
                self.popup = Some(popup);
                self.headline = Headline::default();
            }
        };

        terminal.draw(|frame| self.draw(frame, notifications, list_state))?;
        Ok(())
    }
}

async fn handle_action(
    tx: mpsc::Sender<Message>,
    connection: DbConnection,
    message: MessageAction,
    idx: Option<usize>,
    notifications: Vec<Notification>,
) {
    debug!("handle_message {message:?}");
    let res = match message {
        MessageAction::ScoreIncrement(inc) => {
            let res = update_score(connection, idx, &notifications, inc).await;
            tx.send(Message::Ui(MessageUi::Redraw))
                .await
                .expect("cannot send");
            res
        }
        MessageAction::MarkAsDone => {
            let res = mark_as_done(connection, idx, &notifications).await;
            tx.send(Message::Ui(MessageUi::Redraw))
                .await
                .expect("cannot send");
            res
        }
        MessageAction::MarkBelowAsDone => {
            tx.send(Message::Ui(MessageUi::Info("mark as read...".into())))
                .await
                .expect("cannot send");
            let res = mark_all_below_as_done(connection, idx, &notifications).await;
            tx.send(Message::Ui(MessageUi::Info("mark as read complete".into())))
                .await
                .expect("cannot send");
            res
        }
        MessageAction::Open => {
            let res = open_gh(connection, idx, &notifications).await;
            tx.send(Message::Ui(MessageUi::Redraw))
                .await
                .expect("cannot send");
            res
        }
        MessageAction::Sync => {
            tx.send(Message::Ui(MessageUi::Info("syncing...".into())))
                .await
                .expect("cannot send");

            let res = sync(connection).await;

            tx.send(Message::Ui(MessageUi::Info("sync done".into())))
                .await
                .expect("cannot send");
            res
        }
        MessageAction::SyncBackground => {
            let res = sync(connection).await;
            tx.send(Message::Ui(MessageUi::Redraw))
                .await
                .expect("cannot send");
            res
        }
        MessageAction::Explain => match explain(idx, &notifications).await {
            Ok(explanation) => {
                tx.send(Message::Ui(MessageUi::Popup(Popup {
                    title: "Explain".into(),
                    content: explanation,
                })))
                .await
                .expect("cannot send");
                Ok(())
            }
            Err(e) => Err(e),
        },
        MessageAction::Help => {
            tx.send(Message::Ui(MessageUi::Popup(Popup {
                title: "Help".into(),
                content: KEYMAP.into(),
            })))
            .await
            .expect("cannot send");
            Ok(())
        }
        MessageAction::Quit => Ok(()), // handled in loop break
    };

    if let Err(err) = res {
        _ = tx.send(Message::Ui(MessageUi::Error(err))).await;
    }
}

fn handle_input_loop(tx: mpsc::Sender<Message>) {
    loop {
        let event = event::read();
        if let Err(err) = event {
            tx.blocking_send(Message::Ui(MessageUi::Error(err.to_string())))
                .expect("cannot send");
            return;
        }

        if let Ok(Event::Key(key)) = event {
            let message = match key.code {
                KeyCode::Down => Message::Ui(MessageUi::MoveDown(1)),
                KeyCode::PageDown => Message::Ui(MessageUi::MoveDown(10)),
                KeyCode::Up => Message::Ui(MessageUi::MoveUp(1)),
                KeyCode::PageUp => Message::Ui(MessageUi::MoveUp(10)),
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
            };

            // send message, it will be executed if popup is inactive
            tx.blocking_send(message).expect("cannot send message");
        }
    }
}

async fn auto_sync_notifs_loop(tx: mpsc::Sender<Message>, pool: Pool<DbConnectionManager>) {
    loop {
        debug!("refreshing notifications");
        let (refresh_delay, need_update) =
            match service::check_update_and_limit(pool.get().unwrap()).await {
                Err(_) => (REFRESH_DELAY_SEC, true),
                Ok(update_status) => {
                    debug!("gh status {update_status:?}");
                    (
                        std::cmp::max(REFRESH_DELAY_SEC, update_status.poll_interval),
                        update_status.need_update,
                    )
                }
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

fn popup_area(area: Rect, lines: u16, columns: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Length(lines)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Length(columns)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}

async fn open_gh(
    connection: DbConnection,
    idx: Option<usize>,
    notifications: &[Notification],
) -> Result<(), String> {
    if let Some(idx) = idx {
        if let Some(notification) = notifications.get(idx) {
            return match open::that(notification.url.clone()) {
                Ok(_) => {
                    mark_as_read(connection, notification).await?;
                    Ok(())
                }
                Err(e) => {
                    error!("{e}");
                    Err(format!("Failed to open browser: {}", e))
                }
            };
        }
    }
    Ok(())
}

async fn mark_as_done(
    connection: DbConnection,
    idx: Option<usize>,
    notifications: &[Notification],
) -> Result<(), String> {
    if let Some(idx) = idx {
        if let Some(notification) = notifications.get(idx) {
            return match service::mark_notification_as_done(connection, notification).await {
                Ok(_) => Ok(()),
                Err(e) => {
                    error!("{e}");
                    Err(format!("Failed to mark as done {}", e))
                }
            };
        }
    }
    Ok(())
}

async fn mark_all_below_as_done(
    connection: DbConnection,
    idx: Option<usize>,
    notifications: &[Notification],
) -> Result<(), String> {
    if let Some(idx) = idx {
        let selected_notifications = notifications.iter().skip(idx).collect::<Vec<_>>();
        return match service::mark_notifications_as_done(connection, &selected_notifications).await
        {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("{e}");
                Err(format!("Failed to mark as done {}", e))
            }
        };
    }
    Ok(())
}

async fn mark_as_read(connection: DbConnection, notification: &Notification) -> Result<(), String> {
    match service::mark_notification_as_read(connection, notification).await {
        Err(e) => {
            error!("{e}");
            Err(format!("Failed to mark as read: {}", e))
        }
        Ok(_) => Ok(()),
    }
}

async fn sync(connection: DbConnection) -> Result<(), String> {
    service::sync(connection).await.map_err(|err| {
        let score_error_msg = match err.downcast_ref::<ScoreError>() {
            Some(ScoreError::RuleFileNotFound) => {
                error!("rule file not found");
                Some("rule file not found".into())
            }
            Some(ScoreError::InvalidToml) => {
                error!("invalid toml");
                Some("invalid toml".into())
            }
            Some(ScoreError::InvalidRule(msg)) => {
                error!("invalid rule {:?}", msg);
                Some(format!("invalid rule {:?}", msg))
            }
            None => None,
        };

        let gh_error_msg = match err.downcast_ref::<GhError>() {
            Some(GhError::MissingToken) => {
                error!("env var GH_TOKEN is missing");
                Some("env var GH_TOKEN is missing".to_string())
            }
            None => None,
        };

        match (gh_error_msg, score_error_msg) {
            (Some(g), _) => g,
            (_, Some(s)) => s,
            (None, None) => {
                error!("error in sync {:?}", err);
                "cannot sync".into()
            }
        }
    })
}

async fn refresh(connection: DbConnection) -> Result<Vec<Notification>> {
    service::get_notifications(connection).await
}

async fn update_score(
    connection: DbConnection,
    idx: Option<usize>,
    notifications: &[Notification],
    modifier: i32,
) -> Result<(), String> {
    if let Some(idx) = idx {
        if let Some(notification) = notifications.get(idx) {
            return match service::update_score(connection, notification, modifier).await {
                Ok(_) => Ok(()),
                Err(err) => {
                    error!("error in score update {:?}", err);
                    Err("cannot update score".into())
                }
            };
        }
    }
    Ok(())
}

async fn explain(idx: Option<usize>, notifications: &[Notification]) -> Result<String, String> {
    if let Some(idx) = idx {
        if let Some(notification) = notifications.get(idx) {
            let res = service::explain(notification)
                .await
                .or(Err(String::from("explain failed")))?;

            let explanation = res.iter().fold(String::new(), |acc, rule| {
                let prefix = if acc.is_empty() {
                    String::from("\n")
                } else {
                    acc
                };
                format!("{prefix}rule:{} score:{}\n", rule.name, rule.score)
            });
            let explanation = if notification.score_boost != 0 {
                format!("{explanation}\nmanual boost:{}", notification.score_boost)
            } else {
                explanation
            };
            let explanation = if explanation.is_empty() {
                "\nThis notification doesn't match any rule".to_string()
            } else {
                explanation
            };

            return Ok(explanation);
        };
    }
    Ok(String::new())
}

impl From<&Notification> for Text<'_> {
    fn from(notification: &Notification) -> Self {
        let icon = match notification.type_ {
            NotificationType::Issue => match notification.state {
                NotificationState::Open => "🐛",
                NotificationState::Resolved => "🦋",
                NotificationState::Canceled => "🪳",
                NotificationState::Draft => unreachable!(),
            },
            NotificationType::Release => "🚢",
            NotificationType::PullRequest => match notification.state {
                NotificationState::Open => "📬",
                NotificationState::Resolved => "📪",
                NotificationState::Canceled => "❌",
                NotificationState::Draft => "📝",
            },
        };
        let time = HumanTime::from(notification.updated_at.and_utc())
            .to_text_en(Accuracy::Rough, Tense::Past);
        let txt = format!(
            "{score:>3} {icon} {time:<15} {author:15} {repo:<30} {title}",
            score = notification.score + notification.score_boost,
            icon = icon,
            time = ellipsis(&time, 15),
            author = ellipsis(&notification.author, 15),
            repo = ellipsis(&notification.repo, 30),
            title = ellipsis(&notification.title, 80),
        );

        let style = Style::default();
        let style = if notification.unread {
            style.add_modifier(Modifier::BOLD)
        } else {
            style
        };

        Text::styled(txt, style)
    }
}

fn ellipsis(txt: &str, max_len: usize) -> String {
    if txt.len() > max_len {
        let substr = &txt[0..max_len - 1];
        let mut res = substr.to_owned();

        res.push('…');
        res
    } else {
        let mut res = txt.to_owned();
        let whitespace = vec![' '; max_len - txt.len()];
        let whitespace: String = whitespace.iter().collect();
        res.push_str(&whitespace);
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ellipsis() {
        assert_eq!(ellipsis("lorem ipsum", 5), "lore…");
        assert_eq!(ellipsis("lorem ipsum", 13), "lorem ipsum  ");
    }

    #[test]
    fn test_keymap() {
        // This test can be flaky...
        assert!(KEYMAP.contains("up/down           | move cursor up or down"));
        assert!(KEYMAP.split("\n").collect::<Vec<&str>>().len() >= 10);
    }
}
