use crate::models::{Notification, NotificationState, NotificationType};
use crate::score::Error as ScoreError;
use crate::service;
use anyhow::Result;
use chrono_humanize::{Accuracy, HumanTime, Tense};
use log::{debug, error, info};
use ratatui::crossterm::event::{self, Event, KeyCode};
use ratatui::style::{Modifier, Style};
use ratatui::text::*;
use ratatui::widgets::ListState;
use ratatui::{
    layout::{Constraint, Layout},
    text::Line,
    widgets::List,
    DefaultTerminal,
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
}

#[derive(Clone, PartialEq, Debug)]
enum MessageUi {
    MoveUp(u16),
    MoveDown(u16),
    Error(String),
    Info(String),
    Redraw,
}

const REFRESH_DELAY_SEC: u64 = 300;
const REDRAW_DELAY_SEC: u64 = 60;

pub async fn run() -> Result<()> {
    let res = _run().await;
    ratatui::restore();
    res
}

async fn _run() -> Result<()> {
    let mut terminal = ratatui::init();
    let mut list_state = ListState::default();
    list_state.select_first();

    let (tx, mut rx) = mpsc::channel::<Message>(32);

    let notifications = refresh().await?;
    terminal.draw(|frame| {
        draw(
            frame,
            &notifications,
            &mut list_state,
            &String::new(),
            &String::new(),
        )
    })?;

    let tx_cloned = tx.clone();
    let notif_handle = tokio::spawn(refresh_notifs_loop(tx.clone()));
    let refresh_handle = tokio::spawn(refresh_ui_loop(tx.clone()));
    std::thread::spawn(|| handle_input_loop(tx_cloned));

    loop {
        let maybe_message = rx.recv().await;
        if let Some(message) = maybe_message {
            match message {
                Message::Action(action) => {
                    if action == MessageAction::Quit {
                        break;
                    }
                    let message_action = action.clone();
                    let notifications = refresh().await?;
                    tokio::spawn(handle_action(
                        tx.clone(),
                        message_action,
                        list_state.selected(),
                        notifications,
                    ));
                }
                Message::Ui(ui) => {
                    // FIXME: fetch notif (in db) for *every* ui event (move up/down, etc.)
                    // it should be done only after a change in the list
                    let notifications = refresh().await?;
                    update_ui(ui, &mut terminal, &mut list_state, &notifications).await?;
                }
                Message::Noop => {}
            }
        }
    }

    notif_handle.abort();
    refresh_handle.abort();

    Ok(())
}

async fn handle_action(
    tx: mpsc::Sender<Message>,
    message: MessageAction,
    idx: Option<usize>,
    notifications: Vec<Notification>,
) {
    debug!("handle_message {message:?}");
    let res = match message {
        MessageAction::ScoreIncrement(inc) => {
            let res = update_score(idx, &notifications, inc).await;
            tx.send(Message::Ui(MessageUi::Redraw))
                .await
                .expect("cannot send");
            res
        }
        MessageAction::MarkAsDone => {
            let res = mark_as_done(idx, &notifications).await;
            tx.send(Message::Ui(MessageUi::Redraw))
                .await
                .expect("cannot send");
            res
        }
        MessageAction::MarkBelowAsDone => {
            tx.send(Message::Ui(MessageUi::Info("mark as read...".into())))
                .await
                .expect("cannot send");
            let res = mark_all_below_as_done(idx, &notifications).await;
            tx.send(Message::Ui(MessageUi::Info("mark as read complete".into())))
                .await
                .expect("cannot send");
            res
        }
        MessageAction::Open => {
            let res = open_gh(idx, &notifications).await;
            tx.send(Message::Ui(MessageUi::Redraw))
                .await
                .expect("cannot send");
            res
        }
        MessageAction::Sync => {
            tx.send(Message::Ui(MessageUi::Info("syncing...".into())))
                .await
                .expect("cannot send");

            let res = sync().await;

            tx.send(Message::Ui(MessageUi::Info("sync done".into())))
                .await
                .expect("cannot send");
            res
        }
        MessageAction::SyncBackground => sync().await,
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
                _ => Message::Noop,
            };
            tx.blocking_send(message).expect("cannot send message");
        }
    }
}

async fn refresh_notifs_loop(tx: mpsc::Sender<Message>) {
    loop {
        debug!("refreshing notifications");
        let (refresh_delay, need_update) = match service::check_update_and_limit().await {
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

async fn refresh_ui_loop(tx: mpsc::Sender<Message>) {
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(REDRAW_DELAY_SEC)).await;
        tx.send(Message::Ui(MessageUi::Redraw))
            .await
            .expect("cannot send");
    }
}

async fn update_ui(
    message: MessageUi,
    terminal: &mut DefaultTerminal,
    list_state: &mut ListState,
    notifications: &Vec<Notification>,
) -> Result<()> {
    let (info, err) = match message {
        MessageUi::MoveUp(mov) => {
            list_state.scroll_up_by(mov);
            (String::new(), String::new())
        }
        MessageUi::MoveDown(mov) => {
            list_state.scroll_down_by(mov);
            (String::new(), String::new())
        }
        MessageUi::Error(err) => (String::new(), err),
        MessageUi::Info(info) => (info, String::new()),
        MessageUi::Redraw => (String::new(), String::new()),
    };

    terminal.draw(|frame| draw(frame, notifications, list_state, &err, &info))?;
    Ok(())
}

fn draw(
    frame: &mut Frame,
    notifications: &Vec<Notification>,
    list_state: &mut ListState,
    error: &String,
    info: &String,
) {
    let status = if error.is_empty() {
        if info.is_empty() {
            format!("Riirview, {} notifs", notifications.len())
        } else {
            info.to_string()
        }
    } else {
        error.to_string()
    };
    let title = Line::from_iter([status]);

    let layout_v = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).spacing(1);
    let [one, two] = layout_v.areas(frame.area());

    frame.render_widget(title, one);
    let list = List::new(notifications).highlight_style(Modifier::REVERSED);
    frame.render_stateful_widget(list, two, list_state);
}

async fn open_gh(idx: Option<usize>, notifications: &[Notification]) -> Result<(), String> {
    if let Some(idx) = idx {
        if let Some(notification) = notifications.get(idx) {
            return match open::that(notification.url.clone()) {
                Ok(_) => {
                    mark_as_read(notification).await?;
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

async fn mark_as_done(idx: Option<usize>, notifications: &[Notification]) -> Result<(), String> {
    if let Some(idx) = idx {
        if let Some(notification) = notifications.get(idx) {
            return match service::mark_notification_as_done(notification).await {
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
    idx: Option<usize>,
    notifications: &[Notification],
) -> Result<(), String> {
    if let Some(idx) = idx {
        let selected_notifications = notifications.iter().skip(idx).collect::<Vec<_>>();
        return match service::mark_notifications_as_done(&selected_notifications).await {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("{e}");
                Err(format!("Failed to mark as done {}", e))
            }
        };
    }
    Ok(())
}

async fn mark_as_read(notification: &Notification) -> Result<(), String> {
    match service::mark_notification_as_read(notification).await {
        Err(e) => {
            error!("{e}");
            Err(format!("Failed to mark as read: {}", e))
        }
        Ok(_) => Ok(()),
    }
}

async fn sync() -> Result<(), String> {
    service::sync()
        .await
        .map_err(|err| match err.downcast_ref::<ScoreError>() {
            Some(ScoreError::RuleFileNotFound) => {
                error!("rule file not found");
                "rule file not found".into()
            }
            Some(ScoreError::InvalidToml) => {
                error!("invalid toml");
                "invalid toml".into()
            }
            Some(ScoreError::InvalidRule(msg)) => {
                error!("invalid rule {:?}", msg);
                format!("invalid rule {:?}", msg)
            }
            None => {
                error!("{}", err);
                "cannot sync".into()
            }
        })
}

async fn refresh() -> Result<Vec<Notification>> {
    service::get_notifications().await
}

async fn update_score(
    idx: Option<usize>,
    notifications: &[Notification],
    modifier: i32,
) -> Result<(), String> {
    if let Some(idx) = idx {
        if let Some(notification) = notifications.get(idx) {
            return match service::update_score(notification, modifier).await {
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

impl From<&Notification> for Text<'_> {
    fn from(notification: &Notification) -> Self {
        let icon = match notification.type_ {
            NotificationType::Issue => "🐛",
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
}
