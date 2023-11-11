use front::category::CategoryContent;
use front::home::AppContent;
use front::Route;
use yew::prelude::*;
use yew_router::prelude::*;

fn switch(routes: Route) -> Html {
    match routes {
        Route::Home => html! { <AppContent/> },
        Route::Category { uid } => html! { <CategoryContent uid={uid} /> },
    }
}

#[function_component]
fn App() -> Html {
    html! {
      <BrowserRouter>
        <Switch<Route> render={switch} />
      </BrowserRouter>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
