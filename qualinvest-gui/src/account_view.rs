use crate::account::{
    delete_account, get_user_accounts, insert_account, update_account, AccountView,
};
use leptos::prelude::*;
use leptos::task::spawn_local;

#[component]
pub fn AccountsTable() -> impl IntoView {
    let (editing_id, set_editing_id) = signal::<Option<i32>>(None);
    let (next_id, set_next_id) = signal(-1);
    let (reload_trigger, set_reload_trigger) = signal(0);

    let accounts_resource = Resource::new(
        move || reload_trigger.get(),
        move |_| async move { get_user_accounts().await },
    );

    let is_admin = move || {
        accounts_resource
            .get()
            .and_then(|result| result.ok())
            .map(|(admin, _)| admin)
            .unwrap_or(false)
    };

    let table_data = move || {
        accounts_resource
            .get()
            .and_then(|result| result.ok())
            .map(|(_, accounts)| accounts)
            .unwrap_or_default()
    };

    let (pending_new_rows, set_pending_new_rows) = signal::<Vec<AccountView>>(Vec::new());

    let add_new_row = move |_| {
        let new_id = next_id.get();
        let new_row = AccountView {
            id: new_id,
            broker: String::new(),
            account_name: String::new(),
            user_name: None,
        };

        set_pending_new_rows.update(|rows| rows.push(new_row));
        set_editing_id.set(Some(new_id));
        set_next_id.set(new_id - 1);
    };

    let combined_data = move || {
        let mut data = table_data();
        data.extend(pending_new_rows.get());
        data
    };

    view! {
        <div class="top-button">
            <img class="icon" width=25 src="plus.svg" on:click=add_new_row />
        </div>
        <Suspense fallback=|| view! { <p>"Loading accounts..."</p> }>
            <table class="table">
                <thead>
                    <tr>
                        <th class="header-cell">"Broker"</th>
                        <th class="header-cell">"Account Name"</th>
                        {move || is_admin().then(|| view! {
                            <th class="header-cell">"User"</th>
                        })}
                        <th class="header-cell">"Actions"</th>
                    </tr>
                </thead>
                <tbody>
                    <For
                        each=combined_data
                        key=|row| row.id
                        children=move |row| {
                            view! {
                                <EditableAccountRow
                                    row=row
                                    is_admin=is_admin
                                    editing_id=editing_id
                                    set_editing_id=set_editing_id
                                    set_reload_trigger=set_reload_trigger
                                    set_pending_new_rows=set_pending_new_rows
                                />
                            }
                        }
                    />
                </tbody>
            </table>
        </Suspense>
    }
}

#[component]
fn EditableAccountRow(
    row: AccountView,
    is_admin: impl Fn() -> bool + Copy + Send + Sync + 'static,
    editing_id: ReadSignal<Option<i32>>,
    set_editing_id: WriteSignal<Option<i32>>,
    set_reload_trigger: WriteSignal<i32>,
    set_pending_new_rows: WriteSignal<Vec<AccountView>>,
) -> impl IntoView {
    let row_id = row.id;
    let (edit_broker, set_edit_broker) = signal(row.broker.clone());
    let (edit_account_name, set_edit_account_name) = signal(row.account_name.clone());
    let (edit_user_name, set_edit_user_name) = signal(row.user_name.clone().unwrap_or_default());

    let is_editing = move || editing_id.get() == Some(row_id);

    view! {
        <tr>
            <Show
                when=is_editing
                fallback=move || {
                    view! {
                        <td class="cell">{edit_broker}</td>
                        <td class="cell">{edit_account_name}</td>
                        {move || is_admin().then(|| view! {
                            <td class="cell">{edit_user_name}</td>
                        })}
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
                                    if row_id > 0 {
                                        spawn_local(async move {
                                            match delete_account(row_id).await {
                                                Ok(_) => {
                                                    log::info!("Account deleted successfully");
                                                    set_reload_trigger.update(|v| *v += 1);
                                                }
                                                Err(e) => log::error!("Failed to delete account: {}", e),
                                            }
                                        });
                                    } else {
                                        set_pending_new_rows.update(|rows| {
                                            rows.retain(|r| r.id != row_id);
                                        });
                                    }
                                    set_editing_id.set(None);
                                }
                            />
                        </td>
                    }
                }
            >
                <td class="cell edit">
                    <input
                        type="text"
                        class="input"
                        prop:value=edit_broker
                        on:input=move |ev| set_edit_broker.set(event_target_value(&ev))
                    />
                </td>
                <td class="cell edit">
                    <input
                        type="text"
                        class="input"
                        prop:value=edit_account_name
                        on:input=move |ev| set_edit_account_name.set(event_target_value(&ev))
                    />
                </td>
                {move || is_admin().then(|| view! {
                    <td class="cell edit">
                        <input
                            type="text"
                            class="input"
                            prop:value=edit_user_name
                            on:input=move |ev| set_edit_user_name.set(event_target_value(&ev))
                        />
                    </td>
                })}
                <td class="button-cell">
                    <img
                        class="icon"
                        width=25
                        src="check.svg"
                        on:click=move |_| {
                            let user_name = if is_admin() {
                                let name = edit_user_name.get();
                                if name.is_empty() { None } else { Some(name) }
                            } else {
                                None
                            };

                            let updated_row = AccountView {
                                id: row_id,
                                broker: edit_broker.get(),
                                account_name: edit_account_name.get(),
                                user_name,
                            };

                            if row_id > 0 {
                                let account_to_update = updated_row.clone();
                                spawn_local(async move {
                                    match update_account(account_to_update).await {
                                        Ok(_) => {
                                            log::info!("Account updated successfully");
                                            set_reload_trigger.update(|v| *v += 1);
                                        }
                                        Err(e) => log::error!("Failed to update account: {}", e),
                                    }
                                });
                            } else {
                                let account_to_insert = updated_row.clone();
                                spawn_local(async move {
                                    match insert_account(account_to_insert).await {
                                        Ok(new_id) => {
                                            log::info!("Account inserted successfully with id {}", new_id);
                                            set_pending_new_rows.update(|rows| {
                                                rows.retain(|r| r.id != row_id);
                                            });
                                            set_reload_trigger.update(|v| *v += 1);
                                        }
                                        Err(e) => log::error!("Failed to insert account: {}", e),
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
                            set_editing_id.set(None);
                        }
                    />
                </td>
            </Show>
        </tr>
    }
}
