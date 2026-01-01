use crate::auth::User;
use crate::quotes::{delete_quotes, update_quotes};
use crate::ticker::{delete_ticker, insert_ticker, update_ticker, TickerView};
use crate::time_range::{TimeRange, TimeRangeSelector};
use leptos::prelude::*;
use leptos::task::spawn_local;

#[component]
pub fn TickersTable(
    tickers: Vec<TickerView>,
    selected_ticker_info: ReadSignal<Option<(i32, String)>>,
    set_selected_ticker_info: WriteSignal<Option<(i32, String)>>,
    selected_asset_info: ReadSignal<Option<(i32, String)>>,
) -> impl IntoView {
    let user = expect_context::<Resource<Option<User>>>();
    let (selected_time_range, set_selected_time_range) = signal(TimeRange::Latest);
    let (table_data, set_table_data) = signal(tickers);
    let (editing_id, set_editing_id) = signal::<Option<i32>>(None);
    let (next_id, set_next_id) = signal(-1);

    let add_new_row = move |_| {
        if let Some((asset_id, _)) = selected_asset_info.get() {
            let new_id = next_id.get();
            let new_row = TickerView {
                id: new_id,
                name: String::new(),
                asset: asset_id,
                currency_iso_code: "EUR".to_string(),
                source: String::new(),
                priority: 0,
                factor: 1.0,
            };

            set_table_data.update(|data| {
                data.push(new_row);
            });

            set_editing_id.set(Some(new_id));
            set_next_id.set(new_id - 1);
        }
    };

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

            <Suspense fallback=|| view! { <></> }>
                {move || {
                    user.get().and_then(|user_data| {
                        user_data.and_then(|u| {
                            if u.is_admin {
                                Some(view! {
                                    <div class="top-button">
                                        <img class="icon" width=25 src="plus.svg" on:click=add_new_row />
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
                    <Suspense fallback=|| view! { <></> }>
                        {move || {
                            user.get().and_then(|user_data| {
                                user_data.and_then(|u| {
                                    if u.is_admin {
                                        Some(view! {
                                            <th class="header-cell">"Actions"</th>
                                        }.into_any())
                                    } else {
                                        None
                                    }
                                })
                            })
                        }}
                    </Suspense>
                </tr>
            </thead>
            <tbody>
                <For
                    each=move || table_data.get()
                    key=|ticker| ticker.id
                    children=move |ticker| {
                        view! {
                            <EditableTickerRow
                                row=ticker
                                editing_id=editing_id
                                set_editing_id=set_editing_id
                                set_table_data=set_table_data
                                selected_ticker_info=selected_ticker_info
                                set_selected_ticker_info=set_selected_ticker_info
                                user=user
                            />
                        }
                    }
                />
            </tbody>
        </table>
        </div>
    }
}

#[component]
fn EditableTickerRow(
    row: TickerView,
    editing_id: ReadSignal<Option<i32>>,
    set_editing_id: WriteSignal<Option<i32>>,
    set_table_data: WriteSignal<Vec<TickerView>>,
    selected_ticker_info: ReadSignal<Option<(i32, String)>>,
    set_selected_ticker_info: WriteSignal<Option<(i32, String)>>,
    user: Resource<Option<User>>,
) -> impl IntoView {
    let row_id = row.id;
    let (edit_name, set_edit_name) = signal(row.name.clone());
    let (edit_source, set_edit_source) = signal(row.source.clone());
    let (edit_currency, set_edit_currency) = signal(row.currency_iso_code.clone());
    let (edit_priority, set_edit_priority) = signal(row.priority);
    let (edit_factor, set_edit_factor) = signal(row.factor);

    let is_editing = move || editing_id.get() == Some(row_id);
    let is_selected = move || {
        if let Some((id, _)) = selected_ticker_info.get() {
            id == row_id
        } else {
            false
        }
    };

    view! {
        <tr class:selected=is_selected>
            <Show
                when=is_editing
                fallback=move || {
                    let ticker_id = row_id;
                    let ticker_name = edit_name.get();
                    let ticker_name_clone1 = ticker_name.clone();
                    let ticker_name_clone2 = ticker_name.clone();
                    let ticker_name_clone3 = ticker_name.clone();
                    let ticker_name_clone4 = ticker_name.clone();
                    let ticker_name_clone5 = ticker_name.clone();
                    view! {
                        <td
                            class="cell"
                            on:click=move |_| {
                                if row_id > 0 {
                                    set_selected_ticker_info.set(Some((ticker_id, ticker_name.clone())));
                                }
                            }
                        >
                            {row_id}
                        </td>
                        <td
                            class="cell"
                            on:click=move |_| {
                                if row_id > 0 {
                                    set_selected_ticker_info.set(Some((ticker_id, ticker_name_clone1.clone())));
                                }
                            }
                        >
                            {edit_name}
                        </td>
                        <td
                            class="cell"
                            on:click=move |_| {
                                if row_id > 0 {
                                    set_selected_ticker_info.set(Some((ticker_id, ticker_name_clone2.clone())));
                                }
                            }
                        >
                            {edit_source}
                        </td>
                        <td
                            class="cell"
                            on:click=move |_| {
                                if row_id > 0 {
                                    set_selected_ticker_info.set(Some((ticker_id, ticker_name_clone3.clone())));
                                }
                            }
                        >
                            {edit_currency}
                        </td>
                        <td
                            class="cell"
                            on:click=move |_| {
                                if row_id > 0 {
                                    set_selected_ticker_info.set(Some((ticker_id, ticker_name_clone4.clone())));
                                }
                            }
                        >
                            {edit_priority}
                        </td>
                        <td
                            class="cell"
                            on:click=move |_| {
                                if row_id > 0 {
                                    set_selected_ticker_info.set(Some((ticker_id, ticker_name_clone5.clone())));
                                }
                            }
                        >
                            {edit_factor}
                        </td>
                        <Suspense fallback=|| view! { <></> }>
                            {move || {
                                user.get().and_then(|user_data| {
                                    user_data.and_then(|u| {
                                        if u.is_admin {
                                            Some(view! {
                                                <td class="button-cell">
                                                    <img
                                                        class="icon"
                                                        width=25
                                                        src="locked.svg"
                                                        on:click=move |_| {
                                                            set_editing_id.set(Some(row_id));
                                                        }
                                                    />
                                                    <img
                                                        class="icon"
                                                        width=25
                                                        src="cross.svg"
                                                        on:click=move |_| {
                                                            let ticker_id = row_id;
                                                            if ticker_id > 0 {
                                                                spawn_local(async move {
                                                                    match delete_ticker(ticker_id).await {
                                                                        Ok(_) => {
                                                                            log::info!("Ticker deleted successfully");
                                                                            set_table_data.update(|data| {
                                                                                data.retain(|r| r.id != ticker_id);
                                                                            });
                                                                            set_selected_ticker_info.set(None);
                                                                        }
                                                                        Err(e) => log::error!("Failed to delete ticker: {}", e),
                                                                    }
                                                                });
                                                            } else {
                                                                set_table_data.update(|data| {
                                                                    data.retain(|r| r.id != ticker_id);
                                                                });
                                                            }
                                                            set_editing_id.set(None);
                                                        }
                                                    />
                                                </td>
                                            }.into_any())
                                        } else {
                                            None
                                        }
                                    })
                                })
                            }}
                        </Suspense>
                    }
                }
            >
                <td class="cell edit">{row_id}</td>
                <td class="cell edit">
                    <input
                        type="text"
                        class="input"
                        prop:value=edit_name
                        on:input=move |ev| set_edit_name.set(event_target_value(&ev))
                    />
                </td>
                <td class="cell edit">
                    <input
                        type="text"
                        class="input"
                        prop:value=edit_source
                        on:input=move |ev| set_edit_source.set(event_target_value(&ev))
                    />
                </td>
                <td class="cell edit">
                    <input
                        type="text"
                        class="input"
                        prop:value=edit_currency
                        on:input=move |ev| set_edit_currency.set(event_target_value(&ev))
                    />
                </td>
                <td class="cell edit">
                    <input
                        type="number"
                        class="input"
                        prop:value=edit_priority
                        on:input=move |ev| {
                            if let Ok(priority) = event_target_value(&ev).parse::<i32>() {
                                set_edit_priority.set(priority);
                            }
                        }
                    />
                </td>
                <td class="cell edit">
                    <input
                        type="number"
                        step="any"
                        class="input"
                        prop:value=edit_factor
                        on:input=move |ev| {
                            if let Ok(factor) = event_target_value(&ev).parse::<f64>() {
                                set_edit_factor.set(factor);
                            }
                        }
                    />
                </td>
                <td class="button-cell">
                    <img
                        class="icon"
                        width=25
                        src="check.svg"
                        on:click=move |_| {
                            let updated_row = TickerView {
                                id: row_id,
                                name: edit_name.get(),
                                asset: row.asset,
                                currency_iso_code: edit_currency.get(),
                                source: edit_source.get(),
                                priority: edit_priority.get(),
                                factor: edit_factor.get(),
                            };

                            if row_id > 0 {
                                let ticker_to_update = updated_row.clone();
                                spawn_local(async move {
                                    match update_ticker(ticker_to_update).await {
                                        Ok(_) => {
                                            log::info!("Ticker updated successfully");
                                            set_table_data.update(|data| {
                                                if let Some(existing_row) = data.iter_mut().find(|r| r.id == row_id) {
                                                    *existing_row = updated_row;
                                                }
                                            });
                                        }
                                        Err(e) => log::error!("Failed to update ticker: {}", e),
                                    }
                                });
                            } else {
                                let ticker_to_insert = updated_row.clone();
                                spawn_local(async move {
                                    match insert_ticker(ticker_to_insert).await {
                                        Ok(new_id) => {
                                            log::info!("Ticker inserted successfully with id {}", new_id);
                                            set_table_data.update(|data| {
                                                if let Some(existing_row) = data.iter_mut().find(|r| r.id == row_id) {
                                                    existing_row.id = new_id;
                                                    *existing_row = TickerView {
                                                        id: new_id,
                                                        ..updated_row
                                                    };
                                                }
                                            });
                                        }
                                        Err(e) => log::error!("Failed to insert ticker: {}", e),
                                    }
                                });
                            }
                            set_editing_id.set(None);
                        }
                    />
                    <img
                        class="icon"
                        width=25
                        src="unlocked.svg"
                        on:click=move |_| {
                            if row_id < 0 {
                                set_table_data.update(|data| {
                                    data.retain(|r| r.id != row_id);
                                });
                            }
                            set_editing_id.set(None);
                        }
                    />
                </td>
            </Show>
        </tr>
    }
}
