use crate::gh::Notification;
use crate::models::{Pr, Repo};
use crate::RiirViewError;
use log::info;

pub fn add_notifications(notifications: Vec<Notification>) -> Result<(), RiirViewError> {
    info!("notifications numbers {}", notifications.len());
    for notification in notifications {
        Repo::insert(&notification.repo())?;
        Pr::insert(
            &notification.title(),
            &notification.url(),
            &notification.repo(),
            &notification.r#type(),
            notification.unread(),
            &notification.updated_at(),
        )?;
    }
    Ok(())
}
