use crate::auth::User;
use crate::ticker::TickerView;
use crate::time_range::{TimeRange, TimeRangeSelector};
use leptos::prelude::*;

#[component]
pub fn TickersTable(
    tickers: Vec<TickerView>,
    selected_ticker_info: ReadSignal<Option<(i32, String)>>,
    set_selected_ticker_info: WriteSignal<Option<(i32, String)>>,
) -> impl IntoView {
    let user = expect_context::<Resource<Option<User>>>();
    let (selected_time_range, set_selected_time_range) = signal(TimeRange::All);

    let on_update = move |_| {
        let time_range = selected_time_range.get();
        log::info!("Update tickers in time range: {:?}", time_range);
        // TODO: Implement update logic
    };

    let on_delete = move |_| {
        let time_range = selected_time_range.get();
        log::info!("Delete tickers in time range: {:?}", time_range);
        // TODO: Implement delete logic
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
                                            selected=selected_time_range
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
