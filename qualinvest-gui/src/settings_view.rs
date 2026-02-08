use crate::settings::{delete_user, get_all_users, insert_user, update_user, UserView};
use leptos::prelude::*;
use leptos::task::spawn_local;

#[component]
pub fn UsersTable() -> impl IntoView {
    let (editing_id, set_editing_id) = signal::<Option<i32>>(None);
    let (next_id, set_next_id) = signal(-1);
    let (reload_trigger, set_reload_trigger) = signal(0);

    let users_resource = Resource::new(
        move || reload_trigger.get(),
        move |_| async move { get_all_users().await },
    );

    let table_data = move || {
        users_resource
            .get()
            .and_then(|result| result.ok())
            .unwrap_or_default()
    };

    let (pending_new_rows, set_pending_new_rows) = signal::<Vec<UserView>>(Vec::new());

    let add_new_row = move |_| {
        let new_id = next_id.get();
        let new_row = UserView {
            id: new_id,
            name: String::new(),
            display: String::new(),
            is_admin: false,
            password: String::new(),
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
        <Suspense fallback=|| view! { <p>"Loading users..."</p> }>
            <table class="table">
                <thead>
                    <tr>
                        <th class="header-cell">"Name"</th>
                        <th class="header-cell">"Display"</th>
                        <th class="header-cell">"Admin"</th>
                        <th class="header-cell">"Password"</th>
                        <th class="header-cell">"Actions"</th>
                    </tr>
                </thead>
                <tbody>
                    <For
                        each=combined_data
                        key=|row| row.id
                        children=move |row| {
                            view! {
                                <EditableUserRow
                                    row=row
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
fn EditableUserRow(
    row: UserView,
    editing_id: ReadSignal<Option<i32>>,
    set_editing_id: WriteSignal<Option<i32>>,
    set_reload_trigger: WriteSignal<i32>,
    set_pending_new_rows: WriteSignal<Vec<UserView>>,
) -> impl IntoView {
    let row_id = row.id;
    let (edit_name, set_edit_name) = signal(row.name.clone());
    let (edit_display, set_edit_display) = signal(row.display.clone());
    let (edit_is_admin, set_edit_is_admin) = signal(row.is_admin);
    let (edit_password, set_edit_password) = signal(String::new());

    let is_editing = move || editing_id.get() == Some(row_id);

    view! {
        <tr>
            <Show
                when=is_editing
                fallback=move || {
                    view! {
                        <td class="cell">{edit_name}</td>
                        <td class="cell">{edit_display}</td>
                        <td class="cell">{move || if edit_is_admin.get() { "Yes" } else { "No" }}</td>
                        <td class="cell">"********"</td>
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
                                            match delete_user(row_id).await {
                                                Ok(_) => {
                                                    log::info!("User deleted successfully");
                                                    set_reload_trigger.update(|v| *v += 1);
                                                }
                                                Err(e) => log::error!("Failed to delete user: {}", e),
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
                        prop:value=edit_name
                        on:input=move |ev| set_edit_name.set(event_target_value(&ev))
                    />
                </td>
                <td class="cell edit">
                    <input
                        type="text"
                        class="input"
                        prop:value=edit_display
                        on:input=move |ev| set_edit_display.set(event_target_value(&ev))
                    />
                </td>
                <td class="cell edit">
                    <input
                        type="checkbox"
                        class="input"
                        prop:checked=edit_is_admin
                        on:change=move |ev| {
                            set_edit_is_admin.set(event_target_checked(&ev));
                        }
                    />
                </td>
                <td class="cell edit">
                    <input
                        type="password"
                        class="input"
                        placeholder=move || {
                            if row_id > 0 { "Leave empty to keep unchanged" } else { "Enter password" }
                        }
                        prop:value=edit_password
                        on:input=move |ev| set_edit_password.set(event_target_value(&ev))
                    />
                </td>
                <td class="button-cell">
                    <img
                        class="icon"
                        width=25
                        src="check.svg"
                        on:click=move |_| {
                            let updated_row = UserView {
                                id: row_id,
                                name: edit_name.get(),
                                display: edit_display.get(),
                                is_admin: edit_is_admin.get(),
                                password: edit_password.get(),
                            };

                            if row_id > 0 {
                                let user_to_update = updated_row.clone();
                                spawn_local(async move {
                                    match update_user(user_to_update).await {
                                        Ok(_) => {
                                            log::info!("User updated successfully");
                                            set_reload_trigger.update(|v| *v += 1);
                                        }
                                        Err(e) => log::error!("Failed to update user: {}", e),
                                    }
                                });
                            } else {
                                let user_to_insert = updated_row.clone();
                                spawn_local(async move {
                                    match insert_user(user_to_insert).await {
                                        Ok(new_id) => {
                                            log::info!("User inserted successfully with id {}", new_id);
                                            set_pending_new_rows.update(|rows| {
                                                rows.retain(|r| r.id != row_id);
                                            });
                                            set_reload_trigger.update(|v| *v += 1);
                                        }
                                        Err(e) => log::error!("Failed to insert user: {}", e),
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
