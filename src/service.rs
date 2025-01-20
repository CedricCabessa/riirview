use crate::models::Notification as DBNotification;
use crate::*;
use chrono::NaiveDateTime;
use diesel::dsl::insert_into;
use diesel::prelude::*;
use diesel::update;
use log::{error, info};
use schema::notifications::dsl::*;

pub async fn need_update() -> Result<bool, Box<dyn std::error::Error>> {
    let connection = &mut establish_connection();
    let last_update = get_recent_update(connection);
    gh::need_update(last_update).await
}

pub async fn sync() -> Result<(), Box<dyn std::error::Error>> {
    let connection = &mut establish_connection();
    let last_update = get_recent_update(connection);
    let gh_notifications = gh::fetch_notifications(last_update).await?;

    for gh_notification in gh_notifications {
        let computed_score = compute_score(&gh_notification);
        let db_notification = DBNotification {
            id: gh_notification.id().to_owned(),
            title: gh_notification.title().to_owned(),
            url: gh_notification.url().to_owned(),
            type_: gh_notification.r#type().to_owned(),
            repo: gh_notification.repo().to_owned(),
            unread: gh_notification.unread(),
            updated_at: gh_notification.updated_at(),
            done: false,
            score: computed_score,
        };
        let res = insert_into(notifications)
            .values(&db_notification)
            .on_conflict(id)
            .do_update()
            .set(&db_notification)
            .execute(connection);
        if res.is_err() {
            error!(
                "insert err {} {:?}",
                res.expect_err("no error").to_string(),
                db_notification
            )
        }
    }
    Ok(())
}

pub async fn get_notifications() -> Result<Vec<DBNotification>, Box<dyn std::error::Error>> {
    let connection = &mut establish_connection();
    Ok(notifications
        .select(DBNotification::as_select())
        .filter(done.eq(false))
        .order_by((score.desc(), updated_at.desc()))
        .load(connection)?)
}

pub async fn mark_notification_as_done(
    notification: &DBNotification,
) -> Result<(), Box<dyn std::error::Error>> {
    let connection = &mut establish_connection();
    gh::mark_as_done(&notification.id).await?;
    update(notification)
        .set(done.eq(true))
        .execute(connection)?;
    Ok(())
}

pub async fn mark_notification_as_read(
    notification: &DBNotification,
) -> Result<(), Box<dyn std::error::Error>> {
    let connection = &mut establish_connection();
    gh::mark_as_read(&notification.id).await?;
    update(notification)
        .set(unread.eq(false))
        .execute(connection)?;
    Ok(())
}

fn get_recent_update(connection: &mut SqliteConnection) -> Option<NaiveDateTime> {
    let recent_pr = notifications
        .select(DBNotification::as_select())
        .order_by(updated_at.desc())
        .first(connection);
    let last_update = recent_pr.map(|notif| notif.updated_at).ok();
    info!("recent pr {:?}", last_update);

    last_update
}

fn compute_score(notification: &gh::Notification) -> i32 {
    0
}
