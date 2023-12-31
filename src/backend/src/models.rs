use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use std::collections::HashMap;
use std::env;
use uuid::Uuid;

use crate::RiirViewError;
use libriirview::json::Category as CategoryJson;
use libriirview::json::Pr as PrJson;
use libriirview::json::Repo as RepoJson;

#[derive(Queryable, Selectable, Insertable, Identifiable, Debug)]
#[diesel(table_name = crate::schema::categories)]
pub struct Category {
    id: i32,
    uid: String,
    name: String,
}

#[derive(Queryable, Selectable, Insertable, Associations, Identifiable, Debug)]
#[diesel(belongs_to(Category))]
#[diesel(table_name = crate::schema::repos)]
pub struct Repo {
    id: i32,
    name: String,
    category_id: Option<i32>,
}

impl From<Repo> for RepoJson {
    fn from(repo: Repo) -> RepoJson {
        RepoJson {
            name: repo.name,
            category_id: repo.category_id,
        }
    }
}

#[derive(Queryable, Selectable, Insertable, Associations, Identifiable, Debug)]
#[diesel(belongs_to(Repo))]
#[diesel(table_name = crate::schema::prs)]
pub struct Pr {
    id: i32,
    title: String,
    url: String,
    repo_id: i32,
    type_: String,
    unread: bool,
    updated_at: String,
}

impl From<Pr> for PrJson {
    fn from(pr: Pr) -> PrJson {
        PrJson {
            title: pr.title,
            url: pr.url,
            updated_at: pr.updated_at,
        }
    }
}

impl Category {
    pub fn all() -> Vec<Category> {
        use crate::schema::categories::dsl::*;

        let connection = &mut establish_connection();

        categories
            .select(Category::as_select())
            .load(connection)
            .expect("cannot select Category")
    }

    pub fn create(name_: String) -> Result<Category, RiirViewError> {
        use crate::schema::categories::dsl::*;

        let connection = &mut establish_connection();

        let uid_ = Uuid::new_v4().to_string();

        diesel::insert_into(categories)
            .values((uid.eq(&uid_), name.eq(&name_)))
            .execute(connection)?;

        let cat = categories
            .filter(uid.eq(&uid_))
            .select(Category::as_select())
            .first(connection)?;
        Ok(cat)
    }

    pub fn edit_name(uid_: &String, name_: &String) -> Result<(), RiirViewError> {
        use crate::schema::categories::dsl::*;

        let connection = &mut establish_connection();
        diesel::update(categories)
            .filter(uid.eq(&uid_))
            .set(name.eq(name_))
            .execute(connection)?;

        Ok(())
    }

    pub fn edit_repos(uid_: &String, repos_: &Vec<String>) -> Result<(), RiirViewError> {
        use crate::schema::repos::dsl::*;
        let connection = &mut establish_connection();

        let cat = Category::find_by_uid(uid_)?;

        for repo in repos_ {
            diesel::update(repos.filter(name.eq(repo)))
                .set(category_id.eq(cat.id))
                .execute(connection)?;
        }
        Ok(())
    }

    pub fn find_by_uid(uid_: &String) -> Result<Category, RiirViewError> {
        use crate::schema::categories::dsl::*;
        let connection = &mut establish_connection();
        let cat = categories
            .filter(uid.eq(&uid_))
            .select(Category::as_select())
            .first(connection)?;

        Ok(cat)
    }

    pub fn delete_by_uid(uid_: String) -> Result<(), RiirViewError> {
        use crate::schema::categories::dsl::*;
        let connection = &mut establish_connection();
        diesel::delete(categories.filter(uid.eq(&uid_))).execute(connection)?;
        Ok(())
    }
}

impl From<Category> for CategoryJson {
    fn from(cat: Category) -> CategoryJson {
        CategoryJson {
            uid: cat.uid,
            name: cat.name,
        }
    }
}

impl Repo {
    pub fn by_category(cat: &Category) -> Result<Vec<Repo>, RiirViewError> {
        let connection = &mut establish_connection();

        use crate::schema::categories::dsl::*;

        let cat = categories
            .filter(id.eq(cat.id))
            .select(Category::as_select())
            .first(connection)?;

        use crate::schema::repos;
        let repo_list = Repo::belonging_to(&cat)
            .select(Repo::as_select())
            .order(repos::name.desc())
            .load(connection)?;

        Ok(repo_list)
    }

    pub fn without_category() -> Result<Vec<Repo>, RiirViewError> {
        let connection = &mut establish_connection();

        use crate::schema::repos::dsl::*;

        let repo_list = repos
            .filter(category_id.is_null())
            .select(Repo::as_select())
            .load(connection)?;

        Ok(repo_list)
    }

    pub fn insert(name_: &String) -> Result<(), RiirViewError> {
        use crate::schema::repos::dsl::*;
        let connection = &mut establish_connection();

        let repo = repos
            .filter(name.eq(name_))
            .select(Repo::as_select())
            .first(connection)
            .optional()?;

        if repo.is_none() {
            diesel::insert_into(repos)
                .values(name.eq(name_))
                .execute(connection)?;
        }
        Ok(())
    }
}

impl Pr {
    pub fn insert(
        title_: &String,
        url_: &String,
        repo_: &String,
        type_in: &String,
        unread_: bool,
        updated_at_: &String,
    ) -> Result<(), RiirViewError> {
        use crate::schema::prs::dsl::*;
        use crate::schema::repos::dsl::*;
        let connection = &mut establish_connection();

        let pr = prs
            .filter(url.eq(url_))
            .select(Pr::as_select())
            .first(connection)
            .optional()?;

        if let Some(pr) = pr {
            diesel::update(&pr)
                .set((updated_at.eq(updated_at_), unread.eq(unread_)))
                .execute(connection)?;
        } else {
            let repo = repos
                .filter(name.eq(repo_))
                .select(Repo::as_select())
                .first(connection)?;

            diesel::insert_into(prs)
                .values((
                    title.eq(title_),
                    url.eq(url_),
                    type_.eq(type_in),
                    repo_id.eq(repo.id),
                    unread.eq(unread_),
                    updated_at.eq(updated_at_),
                ))
                .execute(connection)?;
        }
        Ok(())
    }

    pub fn by_category(category: &Category) -> Result<HashMap<String, Vec<Pr>>, RiirViewError> {
        let mut res = HashMap::new();
        let repos = Repo::by_category(category)?;

        let connection = &mut establish_connection();
        use crate::schema::prs::dsl::*;

        for repo in repos {
            let prs_list = Pr::belonging_to(&repo)
                .filter(type_.eq("PullRequest"))
                .filter(unread.eq(true))
                .select(Pr::as_select())
                .order(updated_at.desc())
                .load(connection)?;
            res.insert(repo.name, prs_list);
        }
        Ok(res)
    }

    pub fn uncategorized() -> Result<HashMap<String, Vec<Pr>>, RiirViewError> {
        use crate::schema::prs::dsl::*;
        use crate::schema::repos::dsl::*;
        let connection = &mut establish_connection();
        let mut res: HashMap<String, Vec<Pr>> = HashMap::new();

        let prs_list = prs
            .inner_join(repos)
            .filter(category_id.is_null())
            .filter(type_.eq("PullRequest"))
            .order(repo_id.desc())
            .order(updated_at.desc())
            .select((Pr::as_select(), Repo::as_select()))
            .load::<(Pr, Repo)>(connection)?;

        for (pr, repo) in prs_list {
            match res.get_mut(&repo.name) {
                Some(pr_list) => pr_list.push(pr),
                None => {
                    res.insert(repo.name, vec![pr]);
                }
            }
        }

        Ok(res)
    }

    pub fn last_update() -> Result<Option<String>, RiirViewError> {
        use crate::schema::prs::dsl::*;
        let connection = &mut establish_connection();

        let recent_pr = prs
            .select(Pr::as_select())
            .order(updated_at.desc())
            .first(connection)
            .optional()?;

        match recent_pr {
            Some(recent_pr) => Ok(Some(recent_pr.updated_at)),
            None => Ok(None),
        }
    }
}

pub fn establish_connection() -> SqliteConnection {
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    SqliteConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}
