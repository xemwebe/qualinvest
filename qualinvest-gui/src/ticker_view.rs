use crate::ticker::TickerView;
use leptos::prelude::*;

#[component]
pub fn TickersTable(
    tickers: Vec<TickerView>,
    selected_ticker_id: ReadSignal<Option<i32>>,
    set_selected_ticker_id: WriteSignal<Option<i32>>,
) -> impl IntoView {
    view! {
        <table class="table">
            <thead>
                <tr>
                    <th class="header-cell">"ID"</th>
                    <th class="header-cell">"Name"</th>
                    <th class="header-cell">"Source"</th>
                    <th class="header-cell">"Currency"</th>
                    <th class="header-cell">"Priority"</th>
                    <th class="header-cell">"Factor"</th>
                </tr>
            </thead>
            <tbody>
                <For
                    each=move || tickers.clone()
                    key=|ticker| ticker.id
                    children=move |ticker| {
                        let ticker_id = ticker.id;
                        let is_selected = move || selected_ticker_id.get() == Some(ticker_id);
                        view! {
                            <tr
                                class:selected=is_selected
                                on:click=move |_| {
                                    set_selected_ticker_id.set(Some(ticker_id));
                                }
                            >
                                <td class="cell">{ticker.id}</td>
                                <td class="cell">{ticker.name}</td>
                                <td class="cell">{ticker.source}</td>
                                <td class="cell">{ticker.currency_iso_code}</td>
                                <td class="cell">{ticker.priority}</td>
                                <td class="cell">{ticker.factor}</td>
                            </tr>
                        }
                    }
                />
            </tbody>
        </table>
    }
}
