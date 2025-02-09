use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::{
    backend::Backend, deserialize, serialize, sql_types::VarChar, AsExpression, FromSqlRow,
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
        <String as deserialize::FromSql<VarChar, B>>::from_sql(bytes).map(|sql| {
            match sql.as_str() {
                "PullRequest" => NotificationType::PullRequest,
                "Issue" => NotificationType::Issue,
                "Release" => NotificationType::Release,
                _ => panic!("invalid type {sql}"),
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
        <String as deserialize::FromSql<VarChar, B>>::from_sql(bytes).map(|sql| {
            match sql.as_str() {
                "Draft" => NotificationState::Draft,
                "Resolved" => NotificationState::Resolved,
                "Canceled" => NotificationState::Canceled,
                "Open" => NotificationState::Open,
                _ => panic!("invalid state {sql}"),
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
}

impl Notification {
    pub fn org(&self) -> String {
        self.repo.split('/').next().unwrap().to_string()
    }
}
