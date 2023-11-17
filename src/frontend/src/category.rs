use gloo_net::http::Request;
use gloo_net::Error;
use libriirview::json::Pr;
use std::collections::HashMap;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct CategorieProps {
    pub uid: String,
}

#[function_component(CategoryContent)]
pub fn category_content(CategorieProps { uid }: &CategorieProps) -> HtmlResult {
    let prs = use_state(|| Ok(HashMap::new()));
    let uid = uid.clone();
    {
        let prs = prs.clone();
        use_effect_with((), move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                match fetch_prs(&format!("http://localhost:8000/categories/{}/prs", uid)).await {
                    Ok(fetched_prs) => prs.set(Ok(fetched_prs)),
                    Err(e) => prs.set(Err(e)),
                };
            });
        });
    }

    let html = match &(*prs) {
        Ok(state) => {
            let prs: HashMap<String, Vec<Pr>> = (*state)
                .iter()
                .map(|(k, v)| ((*k).clone(), (*v).clone()))
                .collect();

            html! {<ReposPrs repos_prs={prs.clone()} />}
        }
        Err(e) => html! {<p>{e.to_string()}</p>},
    };
    Ok(html)
}

async fn fetch_prs(url: &str) -> Result<HashMap<String, Vec<Pr>>, Error> {
    let prs = Request::get(url).send().await?.json().await?;
    Ok(prs)
}

#[derive(Properties, PartialEq)]
struct ReposPrsProps {
    repos_prs: HashMap<String, Vec<Pr>>,
}

#[derive(Properties, PartialEq)]
struct PrsProps {
    prs: Vec<Pr>,
}

#[function_component(ReposPrs)]
#[rustfmt::skip::macros(html)]
fn repos_prs_category(ReposPrsProps { repos_prs }: &ReposPrsProps) -> Html {
    repos_prs
        .iter()
        .map(|(repo, prs)| {
            html! {
              <nav class="panel">
                <p class={classes!("panel-heading")}>{repo}</p>
		<PrsContent prs={prs.clone()}/>
              </nav>
            }
        })
        .collect()
}

#[function_component(PrsContent)]
#[rustfmt::skip::macros(html)]
fn prs_content(PrsProps { prs }: &PrsProps) -> Html {
    prs.iter()
        .map(|pr| {
            html! {
              <p class={classes!("panel-block")}>
                <a href={pr.url.clone()}>{pr.title.clone()}</a>
                <div class={classes!("ml-auto")}>{pr.updated_at.clone()}</div>
              </p>
	    }
        })
        .collect()
}
