use crate::models::Notification;
use crate::service;
use log::error;
use ratatui::crossterm::event::{self, Event, KeyCode};
use ratatui::style::{Modifier, Style};
use ratatui::text::*;
use ratatui::widgets::ListState;
use ratatui::{
    layout::{Constraint, Layout},
    text::Line,
    widgets::List,
};

use ratatui::Frame;

pub async fn run(notifications: &mut Vec<Notification>) -> Result<(), Box<dyn std::error::Error>> {
    let mut terminal = ratatui::init();
    let mut list_state = ListState::default();
    list_state.select_first();

    let mut error = String::new();
    let mut info = String::new();

    if need_update().await? {
        info = "New notifications available, press 'g' to update".to_string();
    }
    loop {
        terminal.draw(|frame| draw(frame, notifications, &mut list_state, &error, &info))?;

        if let Event::Key(key) = event::read()? {
            let res = match key.code {
                KeyCode::Char('q') => break,
                KeyCode::Down => {
                    list_state.select_next();
                    info.clear();
                    Ok(())
                }
                KeyCode::PageDown => {
                    list_state.scroll_down_by(10);
                    info.clear();
                    Ok(())
                }
                KeyCode::Up => {
                    list_state.select_previous();
                    info.clear();
                    Ok(())
                }
                KeyCode::PageUp => {
                    list_state.scroll_down_by(10);
                    info.clear();
                    Ok(())
                }

                KeyCode::Enter => open_gh(list_state.selected(), notifications).await,
                KeyCode::Char('r') => mark_as_done(list_state.selected(), notifications).await,
                KeyCode::Char('g') => sync().await,
                _ => Ok(()),
            };
            match res {
                Ok(_) => error.clear(),
                Err(msg) => error = msg,
            }
            refresh(notifications).await?;
        }
    }
    ratatui::restore();
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
            "Riirview".to_string()
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
                Err(e) => Err(format!("Failed to open browser: {}", e)),
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
                Err(e) => Err(format!("Failed to mark as done {}", e)),
            };
        }
    }
    Ok(())
}

async fn mark_as_read(notification: &Notification) -> Result<(), String> {
    match service::mark_notification_as_read(notification).await {
        Err(e) => Err(format!("Failed to mark as read: {}", e)),
        Ok(_) => Ok(()),
    }
}

async fn sync() -> Result<(), String> {
    service::sync().await.map_err(|err| {
        error!("{}", err);
        "cannot sync"
    })?;
    Ok(())
}

async fn refresh(notifications: &mut Vec<Notification>) -> Result<(), String> {
    match service::get_notifications().await {
        Ok(notif) => {
            *notifications = notif;
            Ok(())
        }
        Err(_) => Err("cannot get notifications".into()),
    }
}

async fn need_update() -> Result<bool, String> {
    service::need_update().await.map_err(|err| {
        error!("need update {:?}", err);
        "cannot ask for update".into()
    })
}

//TODO: add add pr number, author, etc
//TODO: proper padding & alignment, test window resize
impl From<&Notification> for Text<'_> {
    fn from(notification: &Notification) -> Self {
        let txt = format!(
            "{:<30} {}",
            notification.repo.clone(),
            notification.title.clone()
        );
        let style = if notification.unread {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(ratatui::style::Color::Gray)
        };
        Text::styled(txt, style)
    }
}
