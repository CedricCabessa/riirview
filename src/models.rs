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
}
