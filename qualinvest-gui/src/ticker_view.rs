use crate::ticker::TickerView;
use leptos::prelude::*;

#[component]
pub fn TickersTable(
    tickers: Vec<TickerView>,
    selected_ticker_info: ReadSignal<Option<(i32, String)>>,
    set_selected_ticker_info: WriteSignal<Option<(i32, String)>>,
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
                        let ticker_name = ticker.name.clone();
                        let is_selected = move || if let Some((id, _)) = selected_ticker_info.get() { id == ticker_id } else { false };
                        view! {
                            <tr
                                class:selected=is_selected
                                on:click=move |_| {
                                    set_selected_ticker_info.set(Some((ticker_id, ticker_name.clone())));
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
