use gloo_net::http::Request;
use gloo_net::Error;
use yew::prelude::*;

use libriirview::json::Category;

#[derive(Properties, PartialEq)]
struct CategoriesProps {
    categories: Vec<Category>,
}

#[function_component(PleaseWait)]
fn please_wait() -> Html {
    html! {<div class="content-area">{"WAIT"}</div>}
}

async fn fetch_category(url: &str) -> Result<Vec<Category>, Error> {
    let categories = Request::get(url).send().await?.json().await?;
    Ok(categories)
}

#[function_component(Categories)]
fn component_category(CategoriesProps { categories }: &CategoriesProps) -> Html {
    categories
        .iter()
        .map(|cat| {
            html! {
            <div key={cat.uid.clone()}>{cat.name.clone()}</div>
            }
        })
        .collect()
}

#[function_component(AppContent)]
fn app_content() -> HtmlResult {
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

#[function_component]
fn App() -> Html {
    let fallback_fn = html! {<PleaseWait/>};
    html! {
        <div>
         <Suspense fallback={fallback_fn}>
              <AppContent  />
            </Suspense>
        </div>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
