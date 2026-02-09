use crate::quotes::QuoteView;
use leptos::prelude::*;

#[component]
pub fn QuotesTable(quotes: Vec<QuoteView>) -> impl IntoView {
    view! {
        <table class="table">
            <thead>
                <tr>
                    <th class="header-cell">"ID"</th>
                    <th class="header-cell">"Price"</th>
                    <th class="header-cell">"Time"</th>
                    <th class="header-cell">"Volume"</th>
                </tr>
            </thead>
            <tbody>
                <For
                    each=move || quotes.clone()
                    key=|quote| quote.id
                    children=move |quote| {
                        view! {
                            <tr>
                                <td class="cell">{quote.id}</td>
                                <td class="cell">{quote.price}</td>
                                <td class="cell">{quote.time.split_once(' ').map(|(date, _)| date.to_string()).unwrap_or(quote.time)}</td>
                                <td class="cell">{quote.volume.map(|v| v.to_string()).unwrap_or_else(|| "N/A".to_string())}</td>
                            </tr>
                        }
                    }
                />
            </tbody>
        </table>
    }
}
