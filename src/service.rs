use crate::dirs;
use crate::models::Notification as DBNotification;
use crate::score::{Rule, Scorer};
use crate::*;
use anyhow::Result;
use anyhow::anyhow;
use chrono::NaiveDateTime;
use diesel::dsl::insert_into;
use diesel::prelude::*;
use diesel::update;
use gh::UpdateStatus;
use log::{debug, error, info};
use models::NotificationState;
use schema::notifications::dsl::*;

pub async fn check_update_and_limit(mut connection: DbConnection) -> Result<UpdateStatus> {
    let last_update = get_recent_update(&mut connection).ok_or(anyhow!("no recent update"))?;
    gh::check_update_and_limit(last_update).await
}

pub async fn sync(connection: &mut DbConnection) -> Result<()> {
    let last_update = get_recent_update(connection);

    let gh_notifications = gh::fetch_notifications(last_update).await?;

    let (gh_prs, gh_releases, gh_issues) = tokio::join!(
        gh::fetch_prs(&gh_notifications),
        gh::fetch_releases(&gh_notifications),
        gh::fetch_issues(&gh_notifications)
    );
    let (gh_prs, gh_releases, gh_issues) = (gh_prs?, gh_releases?, gh_issues?);

    let config = Config::get();
    let scorer = Scorer::new(config.rules_path)?;

    info!("inserting {} notifications", gh_notifications.len());
    for gh_notification in gh_notifications {
        let notif_url = &gh_notification.subject.url.unwrap_or(String::default());
        let (_url, _type, _author, _state) = match gh_notification.subject.r#type {
            gh::NotificationType::PullRequest => {
                let pr = gh_prs
                    .iter()
                    .find(|pr| pr.url == *notif_url)
                    .ok_or(anyhow!("no pr found"))?;

                (
                    pr.html_url.clone(),
                    models::NotificationType::PullRequest,
                    pr.user.login.clone(),
                    if pr.state.as_ref() as &str == "closed" {
                        if pr.merged {
                            NotificationState::Resolved
                        } else {
                            NotificationState::Canceled
                        }
                    } else if pr.draft {
                        NotificationState::Draft
                    } else {
                        NotificationState::Open
                    },
                )
            }
            gh::NotificationType::Release => {
                let release = gh_releases
                    .iter()
                    .find(|release| release.url == *notif_url)
                    .ok_or(anyhow!("no release found"))?;
                (
                    release.html_url.clone(),
                    models::NotificationType::Release,
                    release.author.login.clone(),
                    NotificationState::Open,
                )
            }
            gh::NotificationType::Issue => {
                let issue = gh_issues
                    .iter()
                    .find(|issue| issue.url == *notif_url)
                    .ok_or(anyhow!("no issue found"))?;
                (
                    issue.html_url.clone(),
                    models::NotificationType::Issue,
                    issue.user.login.clone(),
                    if issue.state == "open" {
                        models::NotificationState::Open
                    } else {
                        models::NotificationState::Resolved
                    },
                )
            }
            gh::NotificationType::Unknown => (
                "".into(),
                models::NotificationType::PullRequest,
                "".into(),
                models::NotificationState::Canceled,
            ),
        };
        let mut db_notification = DBNotification {
            id: gh_notification.id,
            reason: gh_notification.reason,
            title: gh_notification.subject.title.trim().into(),
            repo: gh_notification.repository.full_name,
            unread: gh_notification.unread,
            updated_at: gh_notification.updated_at,
            done: false,
            score: -1, // mutable
            score_boost: 0,
            url: _url,
            type_: _type,
            author: _author,
            state: _state,
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
                reason.eq(&db_notification.reason),
                title.eq(&db_notification.title),
                unread.eq(db_notification.unread),
                repo.eq(&db_notification.repo),
                updated_at.eq(db_notification.updated_at),
                done.eq(false),
                score.eq(db_notification.score),
                url.eq(&db_notification.url),
                type_.eq(&db_notification.type_),
                author.eq(&db_notification.author),
                state.eq(&db_notification.state),
            ))
            .execute(connection);
        if res.is_err() {
            error!(
                "insert err {} {:?}",
                res.expect_err("no error"),
                db_notification
            )
        }
    }
    Ok(())
}

pub async fn get_notifications(connection: &mut DbConnection) -> Result<Vec<DBNotification>> {
    Ok(notifications
        .select(DBNotification::as_select())
        .filter(done.eq(false))
        .order_by(((score + score_boost).desc(), updated_at.desc()))
        .load(connection)?)
}

pub async fn mark_notification_as_done(
    connection: &mut DbConnection,
    notification: &DBNotification,
) -> Result<()> {
    gh::mark_as_done(&notification.id).await?;
    update(notification)
        .set(done.eq(true))
        .execute(connection)?;
    Ok(())
}

pub async fn mark_notifications_as_done(
    connection: &mut DbConnection,
    notifs: &Vec<&DBNotification>,
) -> Result<()> {
    let ids = notifs.iter().map(|n| n.id.clone()).collect();
    gh::mark_as_done_multiple(&ids).await?;
    update(notifications)
        .filter(id.eq_any(ids))
        .set(done.eq(true))
        .execute(connection)?;
    Ok(())
}

pub async fn mark_notification_as_read(
    connection: &mut DbConnection,
    notification: &DBNotification,
) -> Result<()> {
    gh::mark_as_read(&notification.id).await?;
    update(notification)
        .set(unread.eq(false))
        .execute(connection)?;
    Ok(())
}

pub async fn update_score(
    connection: &mut DbConnection,
    notification: &DBNotification,
    modifier: i32,
) -> Result<()> {
    update(notification)
        .set(score_boost.eq(notification.score_boost + modifier))
        .execute(connection)?;
    Ok(())
}

pub async fn explain(notification: &DBNotification) -> Result<Vec<Rule>> {
    let directories = dirs::Directories::new();
    let scorer = Scorer::new(directories.config.join("rules.toml"))?;
    let rules = scorer.explain(notification);
    Ok(rules)
}

fn get_recent_update(connection: &mut DbConnection) -> Option<NaiveDateTime> {
    let recent_pr = notifications
        .select(DBNotification::as_select())
        .order_by(updated_at.desc())
        .first(connection);
    let last_update = recent_pr.map(|notif| notif.updated_at).ok();
    info!("recent pr {:?}", last_update);

    last_update
}
