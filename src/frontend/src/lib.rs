pub mod category;
pub mod home;
use yew_router::prelude::*;

#[derive(Clone, Routable, PartialEq)]
pub enum Route {
    #[at("/")]
    Home,
    #[at("/category/:uid")]
    Category { uid: String },
}
