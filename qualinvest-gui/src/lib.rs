pub mod app;
pub mod asset_view;
pub mod assets;
pub mod auth;
pub mod db;
pub mod error_template;
pub mod ticker;
pub mod ticker_view;
pub mod transaction_view;
pub mod transactions;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::*;
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
