use crate::models::Notification;
use crate::score::Error as ScoreError;
use crate::service;
use anyhow::Result;
use chrono_humanize::HumanTime;
use log::{debug, error};
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
use tokio::{select, sync::mpsc};

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
}

#[derive(Clone, PartialEq, Debug)]
enum MessageUi {
    MoveUp(u16),
    MoveDown(u16),
    Error(String),
    Info(String),
    Redraw,
}

pub async fn run() -> Result<()> {
    let res = _run().await;
    ratatui::restore();
    res
}

async fn _run() -> Result<()> {
    let mut terminal = ratatui::init();
    let mut list_state = ListState::default();
    list_state.select_first();

    let mut info = String::new();
    if need_update().await? {
        info = "New notifications available, press 'g' to update".to_string();
    }
    let (tx, mut rx) = mpsc::channel::<Message>(32);

    let notifications = refresh().await?;
    terminal.draw(|frame| {
        draw(
            frame,
            &notifications,
            &mut list_state,
            &String::new(),
            &info,
        )
    })?;

    loop {
        let tx_cloned1 = tx.clone();
        let tx_cloned2 = tx.clone();
        #[rustfmt::skip]
        select! {
            maybe_message = rx.recv() => {
		debug!("receive msg {maybe_message:?}");
		if let Some(message) = maybe_message {
		    match message {
			Message::Action(action) => {
			    if action == MessageAction::Quit {
				break
			    }
			    let message_action = action.clone();
			    let notifications = refresh().await?;
			    tokio::spawn(handle_action(tx_cloned1, message_action, list_state.selected(), notifications));
			},
			Message::Ui(ui) => {
			    // FIXME: fetch notif (in db) for *every* ui event (move up/down, etc.)
			    // it should be done only after a change in the list
			    let notifications = refresh().await?;
			    update_ui(ui, &mut terminal, &mut list_state, &notifications).await?;
			},
			Message::Noop => {}
		    }
		}
            }
            _ = tokio::task::spawn_blocking(|| handle_input(tx_cloned2)) => {}
        }
    }

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
        MessageAction::Quit => Ok(()), // handled in loop break
    };

    if let Err(err) = res {
        _ = tx.send(Message::Ui(MessageUi::Error(err))).await;
    }
}

fn handle_input(tx: mpsc::Sender<Message>) {
    let event = event::read();
    if let Err(err) = event {
        tx.blocking_send(Message::Ui(MessageUi::Error(err.to_string())))
            .expect("cannot send");
        return;
    }

    if let Ok(Event::Key(key)) = event {
        debug!("input {key:?}");
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

async fn need_update() -> Result<bool> {
    service::need_update().await
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
        let icon = match notification.type_.as_ref() {
            "Issue" => "🐛",
            "Release" => "🚢",
            "PullRequest" => match (notification.pr_draft, notification.pr_merged) {
                (true, _) => "📝",
                (false, true) => "📪",
                (false, false) => "📬",
            },
            _ => "❓",
        };
        let txt = format!(
            "{score:>3} {icon} {time:<15} {author:15} {repo:<30} {title}",
            score = notification.score + notification.score_boost,
            icon = icon,
            time = ellipsis(
                &HumanTime::from(notification.updated_at.and_utc()).to_string(),
                15
            ),
            author = ellipsis(&notification.pr_author, 15),
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
