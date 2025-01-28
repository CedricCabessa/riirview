use crate::models::Notification as DBNotification;
use crate::score::Scorer;
use crate::*;
use chrono::NaiveDateTime;
use diesel::dsl::insert_into;
use diesel::prelude::*;
use diesel::update;
use log::{debug, error, info};
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
    let gh_prs = gh::fetch_prs(&gh_notifications).await?;

    // TODO: put the file in xdg compliant folder
    let scorer = Scorer::new("rules.toml")?;

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

pub async fn update_score(
    notification: &DBNotification,
    modifier: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    let connection = &mut establish_connection();
    update(notification)
        .set(score.eq(notification.score + modifier))
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
