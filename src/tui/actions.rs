use crate::gh::Error as GhError;
use crate::models::Notification;
use crate::score::Error as ScoreError;
use crate::tui::messages::{KEYMAP, Message, MessageAction, MessageUi, Popup, UiState};
use crate::{DbConnection, service};
use anyhow::Result;
use log::{debug, error};
use tokio::sync::mpsc;

pub(crate) async fn handle_action(
    tx: mpsc::Sender<Message>,
    mut connection: DbConnection,
    message: MessageAction,
    idx: Option<usize>,
    notifications: Vec<Notification>,
    query: String,
) {
    debug!("handle_message {message:?}");
    let res = match message {
        MessageAction::ScoreIncrement(inc) => {
            let res = update_score(&mut connection, idx, &notifications, inc).await;
            tx.send(Message::Ui(MessageUi::Redraw))
                .await
                .expect("cannot send");
            match res {
                Ok(maybenotif) => {
                    if let Some(notification) = maybenotif {
                        match refresh(&mut connection, &query).await {
                            Ok(refreshed) => {
                                let new_pos = refreshed
                                    .iter()
                                    .position(|it| it.id == notification.id)
                                    .unwrap_or(0);
                                tx.send(Message::Ui(MessageUi::MoveTo(new_pos)))
                                    .await
                                    .expect("cannot send");
                            }
                            Err(e) => {
                                error!("failed to refresh after score update: {}", e);
                            }
                        }
                    }
                    Ok(())
                }
                Err(err) => Err(err),
            }
        }
        MessageAction::MarkAsDone => {
            let res = mark_as_done(&mut connection, idx, &notifications).await;
            tx.send(Message::Ui(MessageUi::Redraw))
                .await
                .expect("cannot send");
            res
        }
        MessageAction::MarkBelowAsDone => {
            tx.send(Message::Ui(MessageUi::UiUpdate(UiState::loading_msg(
                "mark as read...".into(),
            ))))
            .await
            .expect("cannot send");

            let res = mark_all_below_as_done(&mut connection, idx, &notifications).await;

            tx.send(Message::Ui(MessageUi::UiUpdate(UiState::info_msg(
                "mark as read complete".into(),
            ))))
            .await
            .expect("cannot send");

            res
        }
        MessageAction::Open => {
            let res = open_gh(&mut connection, idx, &notifications).await;
            tx.send(Message::Ui(MessageUi::Redraw))
                .await
                .expect("cannot send");
            res
        }
        MessageAction::Sync => {
            tx.send(Message::Ui(MessageUi::UiUpdate(UiState::loading_msg(
                String::new(),
            ))))
            .await
            .expect("cannot send");

            let res = sync(&mut connection).await;

            tx.send(Message::Ui(MessageUi::UiUpdate(UiState::info_msg(
                String::new(),
            ))))
            .await
            .expect("cannot send");

            res
        }
        MessageAction::SyncBackground => {
            tx.send(Message::Ui(MessageUi::UiUpdate(UiState::loading_msg(
                String::new(),
            ))))
            .await
            .expect("cannot send");

            let res = sync(&mut connection).await;

            tx.send(Message::Ui(MessageUi::UiUpdate(UiState::default())))
                .await
                .expect("cannot send");

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
        _ = tx
            .send(Message::Ui(MessageUi::UiUpdate(UiState::error_msg(err))))
            .await;
    }
}

async fn open_gh(
    connection: &mut DbConnection,
    idx: Option<usize>,
    notifications: &[Notification],
) -> Result<(), String> {
    if let Some(idx) = idx
        && let Some(notification) = notifications.get(idx)
    {
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
    Ok(())
}

async fn mark_as_done(
    connection: &mut DbConnection,
    idx: Option<usize>,
    notifications: &[Notification],
) -> Result<(), String> {
    if let Some(idx) = idx
        && let Some(notification) = notifications.get(idx)
    {
        return match service::mark_notification_as_done(connection, notification).await {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("{e}");
                Err(format!("Failed to mark as done {}", e))
            }
        };
    }
    Ok(())
}

async fn mark_all_below_as_done(
    connection: &mut DbConnection,
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

async fn mark_as_read(
    connection: &mut DbConnection,
    notification: &Notification,
) -> Result<(), String> {
    match service::mark_notification_as_read(connection, notification).await {
        Err(e) => {
            error!("{e}");
            Err(format!("Failed to mark as read: {}", e))
        }
        Ok(_) => Ok(()),
    }
}

async fn sync(connection: &mut DbConnection) -> Result<(), String> {
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

pub(crate) async fn refresh(
    connection: &mut DbConnection,
    query: &str,
) -> Result<Vec<Notification>> {
    service::get_notifications(connection, query).await
}

async fn update_score(
    connection: &mut DbConnection,
    idx: Option<usize>,
    notifications: &[Notification],
    modifier: i32,
) -> Result<Option<Notification>, String> {
    if let Some(idx) = idx
        && let Some(notification) = notifications.get(idx)
    {
        return match service::update_score(connection, notification, modifier).await {
            Ok(_) => Ok(Some(notification.clone())),
            Err(err) => {
                error!("error in score update {:?}", err);
                Err("cannot update score".into())
            }
        };
    }
    Ok(None)
}

async fn explain(idx: Option<usize>, notifications: &[Notification]) -> Result<String, String> {
    if let Some(idx) = idx
        && let Some(notification) = notifications.get(idx)
    {
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
    Ok(String::new())
}
