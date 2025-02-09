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
use gh::NotificationType;
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
    // TODO: spawn tasks
    let gh_prs = gh::fetch_prs(&gh_notifications).await?;
    let gh_releases = gh::fetch_releases(&gh_notifications).await?;
    let gh_issues = gh::fetch_issues(&gh_notifications).await?;
    let directories = dirs::Directories::new();
    let scorer = Scorer::new(directories.config.join("rules.toml"))?;

    info!("inserting {} notifications", gh_notifications.len());
    for gh_notification in gh_notifications {
        let pr_number_ = 0;
        let (url_, pr_state_, pr_draft_, pr_merged_, pr_author_) =
            match gh_notification.subject.r#type {
                NotificationType::PullRequest => {
                    let pr = gh_prs.iter().find(|pr| {
                        pr.url
                            == *gh_notification
                                .subject
                                .url
                                .as_ref()
                                .unwrap_or(&String::default())
                    });

                    (
                        pr.map_or(String::new(), |pr| pr.html_url.clone()),
                        pr.map_or(String::new(), |pr| pr.state.clone()),
                        pr.is_some_and(|pr| pr.draft),
                        pr.is_some_and(|pr| pr.merged),
                        pr.map_or(String::new(), |pr| pr.user.login.clone()),
                    )
                }
                NotificationType::Release => {
                    let release = gh_releases.iter().find(|release| {
                        release.url
                            == *gh_notification
                                .subject
                                .url
                                .as_ref()
                                .unwrap_or(&String::default())
                    });
                    (
                        release.map_or(String::new(), |release| release.html_url.clone()),
                        "".into(),
                        false,
                        false,
                        release.map_or(String::new(), |release| release.author.login.clone()),
                    )
                }
                NotificationType::Issue => {
                    let issue = gh_issues.iter().find(|issue| {
                        issue.url
                            == *gh_notification
                                .subject
                                .url
                                .as_ref()
                                .unwrap_or(&String::default())
                    });
                    (
                        issue.map_or(String::new(), |issue| issue.html_url.clone()),
                        String::new(),
                        false,
                        issue.is_some_and(|issue| issue.state == "closed"),
                        issue.map_or(String::new(), |release| release.user.login.clone()),
                    )
                }
            };

        let mut db_notification = DBNotification {
            id: gh_notification.id,
            title: gh_notification.subject.title,
            type_: format!("{:?}", gh_notification.subject.r#type), // FIXME: use Display
            repo: gh_notification.repository.full_name,
            unread: gh_notification.unread,
            updated_at: gh_notification.updated_at,
            done: false,
            score: -1,
            score_boost: 0,
            url: url_,
            // TODO rename those fields
            pr_state: pr_state_,
            pr_number: pr_number_,
            pr_draft: pr_draft_,
            pr_merged: pr_merged_,
            pr_author: pr_author_,
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
