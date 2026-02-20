use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::{
    AsExpression, FromSqlRow, backend::Backend, deserialize, serialize, sql_types::VarChar,
};

#[derive(AsExpression, FromSqlRow, Debug, Clone)]
#[diesel(sql_type = VarChar)]
pub enum NotificationType {
    PullRequest,
    Issue,
    Release,
}

impl<B: Backend> serialize::ToSql<VarChar, B> for NotificationType
where
    str: serialize::ToSql<VarChar, B>,
{
    fn to_sql<'b>(&'b self, out: &mut serialize::Output<'b, '_, B>) -> serialize::Result {
        let type_ = match self {
            NotificationType::PullRequest => "PullRequest",
            NotificationType::Issue => "Issue",
            NotificationType::Release => "Release",
        };
        <str as serialize::ToSql<VarChar, B>>::to_sql(type_, out)
    }
}

impl<B: Backend> deserialize::FromSql<VarChar, B> for NotificationType
where
    String: deserialize::FromSql<VarChar, B>,
{
    fn from_sql(bytes: B::RawValue<'_>) -> deserialize::Result<Self> {
        <String as deserialize::FromSql<VarChar, B>>::from_sql(bytes).and_then(|sql| {
            match sql.as_str() {
                "PullRequest" => Ok(NotificationType::PullRequest),
                "Issue" => Ok(NotificationType::Issue),
                "Release" => Ok(NotificationType::Release),
                _ => Err(format!("invalid type: {sql}").into()),
            }
        })
    }
}

#[derive(AsExpression, FromSqlRow, Debug, Clone)]
#[diesel(sql_type = VarChar)]
pub enum NotificationState {
    Open,
    Draft,
    Resolved, // pr merged, bug solved, ...
    Canceled, // pr closed, wontfix, ...
}

impl<B: Backend> serialize::ToSql<VarChar, B> for NotificationState
where
    str: serialize::ToSql<VarChar, B>,
{
    fn to_sql<'b>(&'b self, out: &mut serialize::Output<'b, '_, B>) -> serialize::Result {
        let state = match self {
            NotificationState::Draft => "Draft",
            NotificationState::Resolved => "Resolved",
            NotificationState::Canceled => "Canceled",
            NotificationState::Open => "Open",
        };
        <str as serialize::ToSql<VarChar, B>>::to_sql(state, out)
    }
}

impl<B: Backend> deserialize::FromSql<VarChar, B> for NotificationState
where
    String: deserialize::FromSql<VarChar, B>,
{
    fn from_sql(bytes: B::RawValue<'_>) -> deserialize::Result<Self> {
        <String as deserialize::FromSql<VarChar, B>>::from_sql(bytes).and_then(|sql| {
            match sql.as_str() {
                "Draft" => Ok(NotificationState::Draft),
                "Resolved" => Ok(NotificationState::Resolved),
                "Canceled" => Ok(NotificationState::Canceled),
                "Open" => Ok(NotificationState::Open),
                _ => Err(format!("invalid state: {sql}").into()),
            }
        })
    }
}

#[derive(Queryable, Selectable, Insertable, Identifiable, AsChangeset, Debug, Clone)]
#[diesel(table_name = crate::schema::notifications)]
pub struct Notification {
    pub id: String,
    pub title: String,
    pub repo: String,
    pub url: String,
    pub type_: NotificationType,
    pub unread: bool,
    pub updated_at: NaiveDateTime,
    pub done: bool,
    pub score: i32,
    pub score_boost: i32,
    pub state: NotificationState,
    pub author: String,
    pub reason: String,
}

impl Notification {
    pub fn org(&self) -> String {
        self.repo.split('/').next().unwrap().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::connection::SimpleConnection;

    fn setup_db() -> SqliteConnection {
        let mut conn = SqliteConnection::establish(":memory:").unwrap();
        conn.batch_execute(
            "CREATE TABLE notifications (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                repo TEXT NOT NULL,
                url TEXT NOT NULL,
                type TEXT NOT NULL,
                state TEXT NOT NULL,
                author TEXT NOT NULL,
                updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                unread BOOLEAN NOT NULL DEFAULT 1,
                done BOOLEAN NOT NULL DEFAULT 0,
                score INTEGER NOT NULL DEFAULT 0,
                score_boost INTEGER NOT NULL DEFAULT 0,
                reason TEXT NOT NULL DEFAULT ''
            )",
        )
        .unwrap();
        conn
    }

    fn insert_notification(conn: &mut SqliteConnection, type_val: &str, state_val: &str) {
        diesel::sql_query(format!(
            "INSERT INTO notifications (id, title, repo, url, type, state, author, reason)
             VALUES ('1', 'test', 'org/repo', 'http://test', '{type_val}', '{state_val}', 'user', 'mention')"
        ))
        .execute(conn)
        .unwrap();
    }

    #[test]
    fn test_unknown_notification_type_returns_error() {
        use crate::schema::notifications::dsl::*;
        let mut conn = setup_db();
        insert_notification(&mut conn, "UnknownType", "Open");
        let result: Result<Vec<Notification>, _> = notifications
            .select(Notification::as_select())
            .load(&mut conn);
        assert!(result.is_err(), "Expected Err for unknown type, but got Ok");
    }

    #[test]
    fn test_unknown_notification_state_returns_error() {
        use crate::schema::notifications::dsl::*;
        let mut conn = setup_db();
        insert_notification(&mut conn, "PullRequest", "UnknownState");
        let result: Result<Vec<Notification>, _> = notifications
            .select(Notification::as_select())
            .load(&mut conn);
        assert!(
            result.is_err(),
            "Expected Err for unknown state, but got Ok"
        );
    }

    #[test]
    fn test_valid_notification_type_roundtrips() {
        use crate::schema::notifications::dsl::*;
        let mut conn = setup_db();
        insert_notification(&mut conn, "PullRequest", "Open");
        let result: Vec<Notification> = notifications
            .select(Notification::as_select())
            .load(&mut conn)
            .unwrap();
        assert_eq!(result.len(), 1);
    }
}
