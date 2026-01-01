use crate::assets::{delete_asset, insert_asset, update_asset, AssetView};
use crate::auth::User;
use leptos::prelude::*;
use leptos::task::spawn_local;

#[component]
pub fn AssetsTable(
    assets: Vec<AssetView>,
    selected_asset_info: ReadSignal<Option<(i32, String)>>,
    set_selected_asset_info: WriteSignal<Option<(i32, String)>>,
    set_selected_ticker_info: WriteSignal<Option<(i32, String)>>,
) -> impl IntoView {
    let user = expect_context::<Resource<Option<User>>>();
    let (table_data, set_table_data) = signal(assets);
    let (editing_id, set_editing_id) = signal::<Option<i32>>(None);
    let (next_id, set_next_id) = signal(-1);

    let add_new_row = move |_| {
        let new_id = next_id.get();
        let new_row = AssetView {
            id: new_id,
            name: String::new(),
            class: String::new(),
        };

        set_table_data.update(|data| {
            data.push(new_row);
        });

        set_editing_id.set(Some(new_id));
        set_next_id.set(new_id - 1);
    };

    view! {
        <div class="assets-container">
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
                        <th class="header-cell">"Class"</th>
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
                        key=|asset| asset.id
                        children=move |asset| {
                            view! {
                                <EditableAssetRow
                                    row=asset
                                    editing_id=editing_id
                                    set_editing_id=set_editing_id
                                    set_table_data=set_table_data
                                    selected_asset_info=selected_asset_info
                                    set_selected_asset_info=set_selected_asset_info
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
fn EditableAssetRow(
    row: AssetView,
    editing_id: ReadSignal<Option<i32>>,
    set_editing_id: WriteSignal<Option<i32>>,
    set_table_data: WriteSignal<Vec<AssetView>>,
    selected_asset_info: ReadSignal<Option<(i32, String)>>,
    set_selected_asset_info: WriteSignal<Option<(i32, String)>>,
    set_selected_ticker_info: WriteSignal<Option<(i32, String)>>,
    user: Resource<Option<User>>,
) -> impl IntoView {
    let row_id = row.id;
    let (edit_name, set_edit_name) = signal(row.name.clone());
    let (edit_class, set_edit_class) = signal(row.class.clone());

    let is_editing = move || editing_id.get() == Some(row_id);
    let is_selected = move || {
        if let Some((id, _)) = selected_asset_info.get() {
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
                    let asset_id = row_id;
                    let asset_name = edit_name.get();
                    let asset_name_clone1 = asset_name.clone();
                    let asset_name_clone2 = asset_name.clone();
                    view! {
                        <td
                            class="cell"
                            on:click=move |_| {
                                if row_id > 0 {
                                    set_selected_asset_info.set(Some((asset_id, asset_name.clone())));
                                    set_selected_ticker_info.set(None);
                                }
                            }
                        >
                            {row_id}
                        </td>
                        <td
                            class="cell"
                            on:click=move |_| {
                                if row_id > 0 {
                                    set_selected_asset_info.set(Some((asset_id, asset_name_clone1.clone())));
                                    set_selected_ticker_info.set(None);
                                }
                            }
                        >
                            {edit_name}
                        </td>
                        <td
                            class="cell"
                            on:click=move |_| {
                                if row_id > 0 {
                                    set_selected_asset_info.set(Some((asset_id, asset_name_clone2.clone())));
                                    set_selected_ticker_info.set(None);
                                }
                            }
                        >
                            {edit_class}
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
                                                            let asset_id = row_id;
                                                            if asset_id > 0 {
                                                                spawn_local(async move {
                                                                    match delete_asset(asset_id).await {
                                                                        Ok(_) => {
                                                                            log::info!("Asset deleted successfully");
                                                                            set_table_data.update(|data| {
                                                                                data.retain(|r| r.id != asset_id);
                                                                            });
                                                                            set_selected_asset_info.set(None);
                                                                            set_selected_ticker_info.set(None);
                                                                        }
                                                                        Err(e) => log::error!("Failed to delete asset: {}", e),
                                                                    }
                                                                });
                                                            } else {
                                                                set_table_data.update(|data| {
                                                                    data.retain(|r| r.id != asset_id);
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
                        prop:value=edit_class
                        on:input=move |ev| set_edit_class.set(event_target_value(&ev))
                    />
                </td>
                <td class="button-cell">
                    <img
                        class="icon"
                        width=25
                        src="check.svg"
                        on:click=move |_| {
                            let updated_row = AssetView {
                                id: row_id,
                                name: edit_name.get(),
                                class: edit_class.get(),
                            };

                            if row_id > 0 {
                                let asset_to_update = updated_row.clone();
                                spawn_local(async move {
                                    match update_asset(asset_to_update).await {
                                        Ok(_) => {
                                            log::info!("Asset updated successfully");
                                            set_table_data.update(|data| {
                                                if let Some(existing_row) = data.iter_mut().find(|r| r.id == row_id) {
                                                    *existing_row = updated_row;
                                                }
                                            });
                                        }
                                        Err(e) => log::error!("Failed to update asset: {}", e),
                                    }
                                });
                            } else {
                                let asset_to_insert = updated_row.clone();
                                spawn_local(async move {
                                    match insert_asset(asset_to_insert).await {
                                        Ok(new_id) => {
                                            log::info!("Asset inserted successfully with id {}", new_id);
                                            set_table_data.update(|data| {
                                                if let Some(existing_row) = data.iter_mut().find(|r| r.id == row_id) {
                                                    existing_row.id = new_id;
                                                    *existing_row = AssetView {
                                                        id: new_id,
                                                        ..updated_row
                                                    };
                                                }
                                            });
                                        }
                                        Err(e) => log::error!("Failed to insert asset: {}", e),
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
