use crate::account::{get_accounts, AccountOption};
use crate::position::{get_performance_graph, get_positions, PositionData, PositionRow};
use crate::time_range::{TimeRange, TimeRangeSelector};
use leptos::prelude::*;
use leptos::wasm_bindgen::JsCast;

#[component]
pub fn PositionTable() -> impl IntoView {
    let (selected_account_ids, set_selected_account_ids) = signal::<Vec<i32>>(Vec::new());
    let (selected_time_range, set_selected_time_range) = signal(TimeRange::All);

    let position_resource = Resource::new(
        move || (selected_account_ids.get(), selected_time_range.get()),
        move |(account_ids, time_range)| async move {
            if account_ids.is_empty() {
                Err(ServerFnError::new("No account selected".to_string()))
            } else {
                get_positions(account_ids, time_range).await
            }
        },
    );

    let performance_graph = Resource::new(
        move || (selected_account_ids.get(), selected_time_range.get()),
        move |(account_ids, time_range)| async move {
            if account_ids.is_empty() {
                None
            } else {
                get_performance_graph(account_ids, time_range).await.ok()
            }
        },
    );

    view! {
        <div class="account-selector">
            <label for="account-select">"Select Account: "</label>
            <Suspense fallback=|| view! { <p>"Loading accounts..."</p> }>
                <Await future=get_accounts()
                    let:accounts
                >
                    {
                        let account_list = accounts.clone();
                        view! {
                            <select
                                id="account-select"
                                multiple
                                on:change=move |ev| {
                                    let select = event_target::<web_sys::HtmlSelectElement>(&ev);
                                    let options = select.selected_options();
                                    let mut ids = Vec::new();
                                    for i in 0..options.length() {
                                        if let Some(opt) = options.item(i) {
                                            if let Ok(opt) = opt.dyn_into::<web_sys::HtmlOptionElement>() {
                                                if let Ok(id) = opt.value().parse::<i32>() {
                                                    ids.push(id);
                                                }
                                            }
                                        }
                                    }
                                    set_selected_account_ids.set(ids);
                                }
                            >
                                <For
                                    each=move || account_list.clone().unwrap_or_default()
                                    key=|account| account.id
                                    children=move |account: AccountOption| {
                                        view! {
                                            <option value=account.id>{account.display_name()}</option>
                                        }
                                    }
                                />
                            </select>
                        }
                    }
                </Await>
            </Suspense>
        </div>
        <div class="time-range-wrapper">
            <TimeRangeSelector set_selected=set_selected_time_range />
        </div>
        <Suspense fallback=|| view! { <p>"Loading performance graph..."</p> }>
            {move || {
                performance_graph.get().flatten().map(|svg| {
                    view! { <div class="performance-graph" inner_html=svg></div> }
                })
            }}
        </Suspense>
        <Suspense fallback=|| view! { <p>"Loading positions..."</p> }>
            {move || {
                position_resource.get().map(|result| {
                    match result {
                        Ok(data) => view! { <PositionDisplay data=data /> }.into_any(),
                        Err(e) => view! { <p class="error">{e.to_string()}</p> }.into_any(),
                    }
                })
            }}
        </Suspense>
    }
}

fn format_f64(value: f64) -> String {
    format!("{:.2}", value)
}

fn format_opt_f64(value: Option<f64>) -> String {
    value.map(|v| format!("{:.2}", v)).unwrap_or_default()
}

#[component]
fn PositionDisplay(data: PositionData) -> impl IntoView {
    let totals = data.totals.clone();
    let cash = data.cash.clone();
    let assets = data.assets.clone();

    view! {
        <table class="table">
            <thead>
                <tr>
                    <th class="header-cell">"Name"</th>
                    <th class="header-cell">"Position"</th>
                    <th class="header-cell">"Last Quote"</th>
                    <th class="header-cell">"Purchase Value"</th>
                    <th class="header-cell">"Trading P&L"</th>
                    <th class="header-cell">"Dividend"</th>
                    <th class="header-cell">"Interest"</th>
                    <th class="header-cell">"Fees"</th>
                    <th class="header-cell">"Tax"</th>
                    <th class="header-cell">"Currency"</th>
                </tr>
            </thead>
            <tbody>
                <For
                    each=move || assets.clone()
                    key=|row| row.name.clone()
                    children=move |row: PositionRow| {
                        view! { <PositionRowView row=row /> }
                    }
                />
                <tr class="cash-row">
                    <td class="cell"><strong>"Cash"</strong></td>
                    <td class="cell">{format_f64(cash.position)}</td>
                    <td class="cell"></td>
                    <td class="cell"></td>
                    <td class="cell">{format_f64(cash.trading_pnl)}</td>
                    <td class="cell">{format_f64(cash.dividend)}</td>
                    <td class="cell">{format_f64(cash.interest)}</td>
                    <td class="cell">{format_f64(cash.fees)}</td>
                    <td class="cell">{format_f64(cash.tax)}</td>
                    <td class="cell">{cash.currency.clone()}</td>
                </tr>
            </tbody>
            <tfoot>
                <tr class="totals-row">
                    <td class="cell"><strong>"Totals"</strong></td>
                    <td class="cell"></td>
                    <td class="cell"></td>
                    <td class="cell"><strong>{format_f64(totals.value)}</strong></td>
                    <td class="cell">{format_f64(totals.trading_pnl)}</td>
                    <td class="cell">{format_f64(totals.dividend)}</td>
                    <td class="cell">{format_f64(totals.interest)}</td>
                    <td class="cell">{format_f64(totals.fees)}</td>
                    <td class="cell">{format_f64(totals.tax)}</td>
                    <td class="cell"></td>
                </tr>
                <tr>
                    <td class="cell"><strong>"Unrealized P&L"</strong></td>
                    <td class="cell" colspan="9">{format_f64(totals.unrealized_pnl)}</td>
                </tr>
            </tfoot>
        </table>
    }
}

#[component]
fn PositionRowView(row: PositionRow) -> impl IntoView {
    view! {
        <tr>
            <td class="cell">{row.name.clone()}</td>
            <td class="cell">{format_f64(row.position)}</td>
            <td class="cell">{format_opt_f64(row.last_quote)}</td>
            <td class="cell">{format_f64(row.purchase_value)}</td>
            <td class="cell">{format_f64(row.trading_pnl)}</td>
            <td class="cell">{format_f64(row.dividend)}</td>
            <td class="cell">{format_f64(row.interest)}</td>
            <td class="cell">{format_f64(row.fees)}</td>
            <td class="cell">{format_f64(row.tax)}</td>
            <td class="cell">{row.currency.clone()}</td>
        </tr>
    }
}
