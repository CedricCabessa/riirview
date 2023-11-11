use crate::Route;
use gloo_net::http::Request;
use gloo_net::Error;
use libriirview::json::Category;
use yew::prelude::*;
use yew_router::prelude::*;

#[function_component(AppContent)]
pub fn app_content() -> HtmlResult {
    let categories = use_state(|| Ok(vec![]));
    {
        let categories = categories.clone();
        use_effect_with((), move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                match fetch_category("http://localhost:8000/categories").await {
                    Ok(fetched_categories) => categories.set(Ok(fetched_categories)),
                    Err(e) => categories.set(Err(e)),
                };
            });
        });
    }

    let html = match &(*categories) {
        Ok(state) => html! {<Categories categories={(*state).clone()} />},
        Err(e) => html! {<p>{e.to_string()}</p>},
    };
    Ok(html)
}

async fn fetch_category(url: &str) -> Result<Vec<Category>, Error> {
    let categories = Request::get(url).send().await?.json().await?;
    Ok(categories)
}

#[derive(Properties, PartialEq)]
struct CategoriesProps {
    categories: Vec<Category>,
}

#[function_component(Categories)]
fn component_category(CategoriesProps { categories }: &CategoriesProps) -> Html {
    categories
        .iter()
        .map(|cat| {
            html! {
		<div key={cat.uid.clone()}><Link<Route> to={Route::Category{ uid: cat.uid.clone()}}>{cat.name.clone()}</Link<Route>></div>
            }
        })
        .collect()
}
