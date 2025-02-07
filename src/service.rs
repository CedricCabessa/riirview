use crate::dirs;
use crate::models::Notification as DBNotification;
use crate::score::Scorer;
use crate::*;
use anyhow::Result;
use chrono::NaiveDateTime;
use diesel::dsl::insert_into;
use diesel::prelude::*;
use diesel::update;
use diesel::upsert::excluded;
use log::{debug, error, info};
use schema::notifications::dsl::*;

pub async fn need_update() -> Result<bool> {
    let connection = &mut establish_connection();
    let last_update = get_recent_update(connection);
    gh::need_update(last_update).await
}

pub async fn sync() -> Result<()> {
    let connection = &mut establish_connection();
    let last_update = get_recent_update(connection);

    let gh_notifications = gh::fetch_notifications(last_update).await?;
    let gh_prs = gh::fetch_prs(&gh_notifications).await?;

    let directories = dirs::Directories::new();
    let scorer = Scorer::new(directories.config.join("rules.toml"))?;

    info!("inserting {} notifications", gh_notifications.len());
    for gh_notification in gh_notifications {
        let pr = gh_prs.iter().find(|pr| pr.url == gh_notification.pr_url());
        let mut db_notification = DBNotification {
            id: gh_notification.id().to_owned(),
            title: gh_notification.title().to_owned(),
            url: gh_notification.url().to_owned(),
            type_: gh_notification.r#type().to_owned(),
            repo: gh_notification.repo().to_owned(),
            unread: gh_notification.unread(),
            updated_at: gh_notification.updated_at(),
            done: false,
            score: -1,
            pr_state: pr.map_or(String::new(), |pr| pr.state.clone()),
            pr_number: pr.map_or(-1, |pr| pr.number),
            pr_draft: pr.is_some_and(|pr| pr.draft),
            pr_merged: pr.is_some_and(|pr| pr.merged),
            pr_author: pr.map_or(String::new(), |pr| pr.user.login.clone()),
            score_boost: 0,
        };
        let computed_score = scorer.score(&db_notification);
        db_notification.score = computed_score;
        debug!(
            "score {} for {} {}",
            computed_score,
            db_notification.title,
            db_notification.url // TODO: display trait
        );
        let res = insert_into(notifications)
            .values(&db_notification)
            .on_conflict(id)
            .do_update()
            .set((
                &db_notification,
                score_boost.eq(excluded(score_boost)),
                done.eq(false),
            ))
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

pub async fn get_notifications() -> Result<Vec<DBNotification>> {
    let connection = &mut establish_connection();
    Ok(notifications
        .select(DBNotification::as_select())
        .filter(done.eq(false))
        .order_by(((score + score_boost).desc(), updated_at.desc()))
        .load(connection)?)
}

pub async fn mark_notification_as_done(notification: &DBNotification) -> Result<()> {
    let connection = &mut establish_connection();
    gh::mark_as_done(&notification.id).await?;
    update(notification)
        .set(done.eq(true))
        .execute(connection)?;
    Ok(())
}

pub async fn mark_notifications_as_done(notifs: &Vec<&DBNotification>) -> Result<()> {
    let connection = &mut establish_connection();
    let ids = notifs.iter().map(|n| n.id.clone()).collect();
    gh::mark_as_done_multiple(&ids).await?;
    update(notifications)
        .filter(id.eq_any(ids))
        .set(done.eq(true))
        .execute(connection)?;
    Ok(())
}

pub async fn mark_notification_as_read(notification: &DBNotification) -> Result<()> {
    let connection = &mut establish_connection();
    gh::mark_as_read(&notification.id).await?;
    update(notification)
        .set(unread.eq(false))
        .execute(connection)?;
    Ok(())
}

pub async fn update_score(notification: &DBNotification, modifier: i32) -> Result<()> {
    let connection = &mut establish_connection();
    update(notification)
        .set(score_boost.eq(notification.score_boost + modifier))
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
