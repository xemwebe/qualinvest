use crate::auth::User;
use crate::quotes::{delete_quotes, update_quotes};
use crate::ticker::TickerView;
use crate::time_range::{TimeRange, TimeRangeSelector};
use leptos::prelude::*;
use leptos::task::spawn_local;

#[component]
pub fn TickersTable(
    tickers: Vec<TickerView>,
    selected_ticker_info: ReadSignal<Option<(i32, String)>>,
    set_selected_ticker_info: WriteSignal<Option<(i32, String)>>,
) -> impl IntoView {
    let user = expect_context::<Resource<Option<User>>>();
    let (selected_time_range, set_selected_time_range) = signal(TimeRange::Latest);

    let on_update = move |_| {
        let time_range = selected_time_range.get();
        if let Some((ticker_id, _)) = selected_ticker_info.get() {
            log::info!(
                "Update quotes for ticker {} in time range: {:?}",
                ticker_id,
                time_range
            );
            spawn_local(async move {
                match update_quotes(ticker_id, time_range).await {
                    Ok(_) => log::info!("Quotes updated successfully"),
                    Err(e) => log::error!("Failed to update quotes: {}", e),
                }
            });
        }
    };

    let on_delete = move |_| {
        let time_range = selected_time_range.get();
        if let Some((ticker_id, _)) = selected_ticker_info.get() {
            log::info!(
                "Delete quotes for ticker {} in time range: {:?}",
                ticker_id,
                time_range
            );
            spawn_local(async move {
                match delete_quotes(ticker_id, time_range).await {
                    Ok(_) => log::info!("Quotes deleted successfully"),
                    Err(e) => log::error!("Failed to delete quotes: {}", e),
                }
            });
        }
    };

    view! {
        <div class="tickers-container">
            <Suspense fallback=|| view! { <></> }>
                {move || {
                    user.get().and_then(|user_data| {
                        user_data.and_then(|u| {
                            if u.is_admin {
                                Some(view! {
                                    <div class="ticker-update">
                                        <TimeRangeSelector
                                            set_selected=set_selected_time_range
                                        />
                                        <div class="action-buttons">
                                            <button
                                                class="btn btn-primary"
                                                on:click=on_update
                                                disabled=move || selected_ticker_info.get().is_none()
                                            >
                                                "Update"
                                            </button>
                                            <button
                                                class="btn btn-danger"
                                                on:click=on_delete
                                                disabled=move || selected_ticker_info.get().is_none()
                                            >
                                                "Delete"
                                            </button>
                                        </div>
                                    </div>
                                }.into_any())
                            } else {
                                None
                            }
                        })
                    })
                }}
            </Suspense>

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
        </div>
    }
}
