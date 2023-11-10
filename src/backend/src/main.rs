#[macro_use]
extern crate rocket;

use backend::gh;
use backend::models::{Category, Pr, Repo};
use backend::service;
use libriirview::json::{
    Category as CategoryJson, CategoryDetail as CategoryDetailJson, Pr as PrJson, Repo as RepoJson,
};
use rocket::response::status::BadRequest;
use rocket::serde::json::Json;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
struct CreateCategory<'a> {
    name: &'a str,
}

#[derive(Deserialize)]
struct UpdateCategory {
    name: Option<String>,
    repos: Option<Vec<String>>,
}

#[get("/categories")]
fn get_categories() -> Json<Vec<CategoryJson>> {
    Json(Category::all().into_iter().map(|c| c.into()).collect())
}

#[post("/categories", data = "<input>")]
fn create_category(input: Json<CreateCategory>) -> Result<Json<CategoryJson>, BadRequest<String>> {
    match Category::create(input.name.to_string()) {
        Ok(cat) => Ok(Json(cat.into())),
        Err(e) => Err(e.into()),
    }
}

#[post("/categories/<uid>", data = "<input>")]
fn update_category(
    uid: String,
    input: Json<UpdateCategory>,
) -> Result<Json<CategoryDetailJson>, BadRequest<String>> {
    if let Some(name) = &input.name {
        Category::edit_name(&uid, name).map_err(|e| e.into())?;
    };

    if let Some(repos) = &input.repos {
        Category::edit_repos(&uid, repos).map_err(|e| e.into())?;
    };

    get_category(uid)
}

#[get("/categories/<uid>")]
fn get_category(uid: String) -> Result<Json<CategoryDetailJson>, BadRequest<String>> {
    let cat = Category::find_by_uid(&uid);

    match cat {
        Ok(cat) => {
            let repos = Repo::by_category(&cat).map_err(|e| e.into())?;
            let repos_json: Vec<RepoJson> = repos.into_iter().map(|r| r.into()).collect();

            Ok(Json(CategoryDetailJson::new(cat.into(), repos_json)))
        }
        Err(e) => Err(e.into()),
    }
}

#[get("/categories/<uid>/prs")]
fn get_prs(uid: String) -> Result<Json<HashMap<String, Vec<PrJson>>>, BadRequest<String>> {
    let cat = Category::find_by_uid(&uid).map_err(|e| e.into())?;

    let pr_by_cat = Pr::by_category(&cat).map_err(|e| e.into())?;
    let pr_by_cat: HashMap<String, Vec<PrJson>> = pr_by_cat
        .into_iter()
        .map(|(repo, prs)| (repo, prs.into_iter().map(|pr| pr.into()).collect()))
        .collect();

    Ok(Json(pr_by_cat))
}

#[delete("/categories/<uid>")]
fn del_category(uid: String) -> Result<(), BadRequest<String>> {
    Category::delete_by_uid(uid).map_err(|e| e.into())
}

#[get("/prs/uncategorized")]
fn get_prs_uncategorized() -> Result<Json<HashMap<String, Vec<PrJson>>>, BadRequest<String>> {
    let pr_by_cat = Pr::uncategorized().map_err(|e| e.into())?;
    let pr_by_cat: HashMap<String, Vec<PrJson>> = pr_by_cat
        .into_iter()
        .map(|(repo, prs)| (repo, prs.into_iter().map(|pr| pr.into()).collect()))
        .collect();

    Ok(Json(pr_by_cat))
}

#[post("/sync")]
async fn sync() -> Result<(), BadRequest<String>> {
    let last_update = Pr::last_update().map_err(|e| e.into())?;

    let res = gh::gh(last_update.as_deref())
        .await
        .map_err(|e| BadRequest(e.to_string()))?;

    service::add_notifications(res).map_err(|e| e.into())?;

    Ok(())
}

#[launch]
fn rocket() -> _ {
    env_logger::init();
    rocket::build()
        .mount("/", routes![get_categories])
        .mount("/", routes![get_category])
        .mount("/", routes![del_category])
        .mount("/", routes![create_category])
        .mount("/", routes![update_category])
        .mount("/", routes![get_prs])
        .mount("/", routes![get_prs_uncategorized])
        .mount("/", routes![sync])
}
