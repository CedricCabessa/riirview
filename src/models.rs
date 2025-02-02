use chrono::NaiveDateTime;
use diesel::prelude::*;

#[derive(Queryable, Selectable, Insertable, Identifiable, AsChangeset, Debug)]
#[diesel(table_name = crate::schema::notifications)]
pub struct Notification {
    pub id: String,
    pub title: String,
    pub repo: String,
    pub url: String,
    pub type_: String,
    pub unread: bool,
    pub updated_at: NaiveDateTime,
    pub done: bool,
    pub score: i32,
    // from pull request
    pub pr_state: String,
    pub pr_number: i32,
    pub pr_draft: bool,
    pub pr_merged: bool,
    pub pr_author: String,
}
