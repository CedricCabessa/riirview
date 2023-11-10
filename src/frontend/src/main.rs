use gloo_net::http::Request;
use yew::prelude::*;

use libriirview::json::Category;

#[function_component(PleaseWait)]
fn please_wait() -> Html {
    html! {<div class="content-area">{"WAIT"}</div>}
}

#[function_component(AppContent)]
fn app_content() -> HtmlResult {
    let categories = use_state(|| vec![]);
    {
        let categories = categories.clone();
        use_effect_with((), move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                let fetch_categories: Vec<Category> =
                    Request::get("http://localhost:8000/categories")
                        .send()
                        .await
                        .unwrap()
                        .json()
                        .await
                        .unwrap();
                categories.set(fetch_categories);
            });
            || ()
        });
    }

    let html = (*categories)
        .iter()
        .map(|c| {
            html! {<p>{c.name.clone()}</p>}
        })
        .collect();
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
