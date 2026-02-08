use leptos::prelude::*;
use leptos_meta::Stylesheet;
use leptos_router::components::*; // Ensure you have this import
use leptos_router::path;

use crate::components::selector::Selector;
use crate::components::system_generator::World;
use crate::components::trade_computer::Trade;

#[component]
pub fn App() -> impl IntoView {
    view! {
        <Stylesheet id="leptos" href="/pkg/worldgen.css" />
        <Router>
            <Routes fallback=|| view! { "Page not found." }>
                // This replaces your if/else logic
                <Route path=path!("/") view=Selector />
                <Route path=path!("/world") view=World />
                <Route path=path!("/trade") view=Trade />
            </Routes>
        </Router>
    }
}
