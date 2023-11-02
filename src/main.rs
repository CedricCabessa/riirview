#[macro_use]
extern crate rocket;

use riirview::gh;
use riirview::models::{Category, CategoryDetail, Pr, Repo};
use riirview::service;
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
fn get_categories() -> Json<Vec<Category>> {
    Json(Category::all())
}

#[post("/categories", data = "<input>")]
fn create_category(input: Json<CreateCategory>) -> Result<Json<Category>, BadRequest<String>> {
    match Category::create(input.name.to_string()) {
        Ok(cat) => Ok(Json(cat)),
        Err(e) => Err(e.into()),
    }
}

#[post("/categories/<uid>", data = "<input>")]
fn update_category(
    uid: String,
    input: Json<UpdateCategory>,
) -> Result<Json<CategoryDetail>, BadRequest<String>> {
    if let Some(name) = &input.name {
        Category::edit_name(&uid, name).map_err(|e| e.into())?;
    };

    if let Some(repos) = &input.repos {
        Category::edit_repos(&uid, repos).map_err(|e| e.into())?;
    };

    get_category(uid)
}

#[get("/categories/<uid>")]
fn get_category(uid: String) -> Result<Json<CategoryDetail>, BadRequest<String>> {
    let cat = Category::find_by_uid(&uid);

    match cat {
        Ok(cat) => {
            let repos = Repo::by_category(&cat).map_err(|e| e.into())?;

            Ok(Json(CategoryDetail::new(cat, repos)))
        }
        Err(e) => Err(e.into()),
    }
}

#[get("/categories/<uid>/prs")]
fn get_prs(uid: String) -> Result<Json<HashMap<String, Vec<Pr>>>, BadRequest<String>> {
    let cat = Category::find_by_uid(&uid).map_err(|e| e.into())?;

    let pr_by_cat = Pr::by_category(&cat).map_err(|e| e.into())?;
    Ok(Json(pr_by_cat))
}

#[delete("/categories/<uid>")]
fn del_category(uid: String) -> Result<(), BadRequest<String>> {
    Category::delete_by_uid(uid).map_err(|e| e.into())
}

#[get("/prs/uncategorized")]
fn get_prs_uncategorized() -> Result<Json<HashMap<String, Vec<Pr>>>, BadRequest<String>> {
    let pr_by_cat = Pr::uncategorized().map_err(|e| e.into())?;
    Ok(Json(pr_by_cat))
}

#[post("/sync")]
async fn sync() -> Result<(), BadRequest<String>> {
    let res = gh::gh()
        .await
        .map_err(|e| BadRequest(Some(e.to_string())))?;

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
