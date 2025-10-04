use crate::transactions::{TransactionDisplay, TransactionView};
use leptos::prelude::*;

#[component]
pub fn TransactionsTable(transactions: Vec<TransactionView>) -> impl IntoView {
    let (table_data, set_table_data) = signal(transactions);

    let (editing_id, set_editing_id) = signal::<Option<i32>>(None);
    let (next_id, set_next_id) = signal(-1);

    let add_new_row = move |_| {
        let new_id = next_id.get();
        let new_row = TransactionView {
            id: new_id,
            group_id: None,
            asset_id: None,
            asset_name: None,
            position: None,
            trans_type: String::new(),
            cash_amount: 0.0,
            cash_currency: "EUR".to_string(),
            cash_date: String::new(),
            note: None,
            account_id: -1,
            state: TransactionDisplay::Edit,
        };

        set_table_data.update(|data| {
            data.push(new_row);
        });

        set_editing_id.set(Some(new_id));
        set_next_id.set(new_id - 1);
    };

    view! {
        <div class="top-button">
            <img class="icon" width=25 src="plus.svg" on:click=add_new_row />
        </div>
        <table class="table">
            <thead>
                <tr>
                    <th class="header-cell">"Group ID"</th>
                    <th class="header-cell">"Asset Name"</th>
                    <th class="header-cell">"Position"</th>
                    <th class="header-cell">"Trans Type"</th>
                    <th class="header-cell">"Cash Amount"</th>
                    <th class="header-cell">"Cash Currency"</th>
                    <th class="header-cell">"Cash Date"</th>
                    <th class="header-cell">"Note"</th>
                    <th class="header-cell">"Actions"</th>
                </tr>
            </thead>
            <tbody>
                <For
                    each=move || table_data.get()
                    key=|row| row.id
                    children=move |row| {
                        view! {
                            <EditableTransactionRow
                                row=row
                                editing_id=editing_id
                                set_editing_id=set_editing_id
                                set_table_data=set_table_data
                            />
                        }
                    }
                />
            </tbody>
        </table>
    }
}

#[component]
fn EditableTransactionRow(
    row: TransactionView,
    editing_id: ReadSignal<Option<i32>>,
    set_editing_id: WriteSignal<Option<i32>>,
    set_table_data: WriteSignal<Vec<TransactionView>>,
) -> impl IntoView {
    let row_id = row.id;
    let (edit_group_id, set_edit_group_id) = signal(row.group_id);
    let (edit_asset_name, set_edit_asset_name) = signal(row.asset_name.clone());
    let (edit_position, set_edit_position) = signal(row.position);
    let (edit_trans_type, set_edit_trans_type) = signal(row.trans_type.clone());
    let (edit_cash_amount, set_edit_cash_amount) = signal(row.cash_amount);
    let (edit_cash_currency, set_edit_cash_currency) = signal(row.cash_currency.clone());
    let (edit_cash_date, set_edit_cash_date) = signal(row.cash_date.clone());
    let (edit_note, set_edit_note) = signal(row.note.clone());

    let is_editing = move || editing_id.get() == Some(row_id);
    view! {
        <tr>
            <Show
                when=is_editing
                fallback=move || {
                    view! {
                            <td class="cell">{edit_group_id}</td>
                            <td class="cell">{edit_asset_name}</td>
                            <td class="cell">{edit_position}</td>
                            <td class="cell">{edit_trans_type}</td>
                            <td class="cell">{edit_cash_amount}</td>
                            <td class="cell">{edit_cash_currency}</td>
                            <td class="cell">{edit_cash_date}</td>
                            <td class="cell">{edit_note}</td>
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
                                        set_table_data.update(|data| {
                                            data.retain(|r| r.id != row_id);
                                        });
                                        set_editing_id.set(None);
                                    }
                                />
                            </td>
                    }
                }
            >
                <td class="cell edit">
                    <input
                        type="number"
                        class="input"
                        prop:value=move || edit_group_id.get().map(|id| id.to_string()).unwrap_or_default()
                        on:input=move |ev| {
                            let value = event_target_value(&ev);
                            set_edit_group_id.set(if value.is_empty() { None } else { value.parse().ok() });
                        }
                    />
                </td>
                <td class="cell edit">
                    <input
                        type="text"
                        class="input"
                        prop:value=move || edit_asset_name.get().unwrap_or_default()
                        on:input=move |ev| {
                            let value = event_target_value(&ev);
                            set_edit_asset_name.set(if value.is_empty() { None } else { Some(value) });
                        }
                    />
                </td>
                <td class="cell edit">
                    <input
                        type="number"
                        step="any"
                        class="input"
                        prop:value=move || edit_position.get().map(|p| p.to_string()).unwrap_or_default()
                        on:input=move |ev| {
                            let value = event_target_value(&ev);
                            set_edit_position.set(if value.is_empty() { None } else { value.parse().ok() });
                        }
                    />
                </td>
                <td class="cell edit">
                    <input
                        type="text"
                        class="input"
                        prop:value=edit_trans_type
                        on:input=move |ev| set_edit_trans_type.set(event_target_value(&ev))
                    />
                </td>
                <td class="cell edit">
                    <input
                        type="number"
                        step="any"
                        class="input"
                        prop:value=edit_cash_amount
                        on:input=move |ev| {
                            if let Ok(amount) = event_target_value(&ev).parse::<f64>() {
                                set_edit_cash_amount.set(amount);
                            }
                        }
                    />
                </td>
                <td class="cell edit">
                    <input
                        type="text"
                        class="input"
                        prop:value=edit_cash_currency
                        on:input=move |ev| set_edit_cash_currency.set(event_target_value(&ev))
                    />
                </td>
                <td class="cell edit">
                    <input
                        type="date"
                        class="input"
                        prop:value=edit_cash_date
                        on:input=move |ev| set_edit_cash_date.set(event_target_value(&ev))
                    />
                </td>
                <td class="cell edit">
                    <input
                        type="text"
                        class="input"
                        prop:value=move || edit_note.get().unwrap_or_default()
                        on:input=move |ev| {
                            let value = event_target_value(&ev);
                            set_edit_note.set(if value.is_empty() { None } else { Some(value) });
                        }
                    />
                </td>
                <td class="button-cell">
                    <img
                        class="icon"
                        width=25
                        src="check.svg"
                        on:click=move |_| {
                            let updated_row = TransactionView {
                                id: row_id,
                                group_id: edit_group_id.get(),
                                asset_id: row.asset_id,
                                asset_name: edit_asset_name.get(),
                                position: edit_position.get(),
                                trans_type: edit_trans_type.get(),
                                cash_amount: edit_cash_amount.get(),
                                cash_currency: edit_cash_currency.get(),
                                cash_date: edit_cash_date.get(),
                                note: edit_note.get(),
                                account_id: 0,
                                state: TransactionDisplay::View,
                            };
                            set_table_data.update(|data| {
                                if let Some(existing_row) = data.iter_mut().find(|r| r.id == row_id) {
                                    *existing_row = updated_row;
                                }
                            });
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
