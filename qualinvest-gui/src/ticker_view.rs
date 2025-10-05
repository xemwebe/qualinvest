use crate::ticker::TickerView;
use leptos::prelude::*;

#[component]
pub fn TickerTable(ticker_list: Vec<TickerView>) -> impl IntoView {
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
                    each=move || ticker_list.clone()
                    key=|ticker| ticker.id
                    children=move |ticker| {
                        view! {
                            <tr>
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
