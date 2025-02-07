use chrono::NaiveDateTime;
use diesel::prelude::*;

#[derive(Queryable, Selectable, Insertable, Identifiable, AsChangeset, Debug, Clone)]
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
    pub score_boost: i32,
    // from pull request
    pub pr_state: String,
    pub pr_number: i32,
    pub pr_draft: bool,
    pub pr_merged: bool,
    pub pr_author: String,
}

impl Notification {
    pub fn org(&self) -> String {
        self.repo.split('/').next().unwrap().to_string()
    }
}
