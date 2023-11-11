use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct Category {
    pub uid: String,
    pub name: String,
}

#[derive(Serialize, Deserialize)]
pub struct CategoryDetail {
    #[serde(flatten)]
    pub category: Category,
    pub repos: Vec<String>,
}

impl CategoryDetail {
    pub fn new(category: Category, repos: Vec<Repo>) -> CategoryDetail {
        CategoryDetail {
            category,
            repos: repos.iter().map(|r| r.name.clone()).collect(),
        }
    }
}

#[derive(Serialize)]
pub struct Repo {
    pub name: String,
    pub category_id: Option<i32>,
}

#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub struct Pr {
    pub title: String,
    pub url: String,
    pub updated_at: String,
}
