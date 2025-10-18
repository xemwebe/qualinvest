use crate::quotes::{get_quotes_graph, QuoteFilter};
use leptos::prelude::*;

#[component]
pub fn QuotesGraph(ticker_id: i32) -> impl IntoView {
    let graph_svg = Resource::new(
        move || ticker_id,
        move |ticker_id| async move { get_quotes_graph(QuoteFilter { ticker_id }).await.ok() },
    );

    view! {
        <div class="quotes-graph">
            <Suspense fallback=move || {
                view! { <p>"Loading graph..."</p> }
            }>
                {move || {
                    match graph_svg.get().flatten() {
                        Some(svg) => {
                            view! {
                                <div inner_html=svg></div>
                            }.into_any()
                        }
                        None => {
                            view! {
                                <div class="error">
                                    <p>"Failed to load graph"</p>
                                </div>
                            }.into_any()
                        }
                    }
                }}
            </Suspense>
        </div>
    }
}
