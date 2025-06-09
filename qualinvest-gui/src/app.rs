use crate::transactions::{TransactionDisplay, TransactionView};
use leptos::ev::MouseEvent;
use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes, A},
    StaticSegment,
};
use leptos_struct_table::*;
use std::ops::Range;

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <AutoReload options=options.clone() />
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    view! {
        // injects a stylesheet into the document <head>
        // id=leptos means cargo-leptos will hot-reload this stylesheet
        <Stylesheet id="leptos" href="/pkg/qualinvest-gui.css"/>

        // sets the document title
        <Title text="QuantInvest"/>

        // content for this welcome page
        <Router>
            <Nav />
            <main>
                <Routes fallback=|| "Page not found.".into_view()>
                    <Route path=StaticSegment("") view=HomePage/>
                    <Route path=StaticSegment("transactions") view=|| { view!{ <Transactions/> } }/>
                    <Route path=StaticSegment("position") view=Position/>
                    <Route path=StaticSegment("assets") view=|| { view!{ <Assets/> } }/>
                    <Route path=StaticSegment("performance") view=|| { view!{ <Performance/> } }/>
                    <Route path=StaticSegment("settings") view=|| { view!{ <Settings/> } }/>
                    <Route path=StaticSegment("accounts") view=|| { view!{ <Accounts/> } }/>
                </Routes>
            </main>
        </Router>
    }
}

/// Renders the home page of your application.
#[component]
fn HomePage() -> impl IntoView {
    view! {
    <div class="center">
    <div class="warning">
        <h1>Please log in!</h1>
    </div>
    <div class="warning,block"> You are logged in as administrator. </div>

    <h1>Quant Invest</h1>
        <div class="inline">
        <p class="block">Quant Invest is a tool to manage a portfolio of investments of common assets
        like shares, bonds or loans.</p>

        <p class="block">The functionality covers basic book-keeping of positions,
        paid fees and tax and calculation of a couple of performance figures,
        eg. realised and unrealised p&l over specific time periods.
        Market data is automatically retreived from various,
        configurable sources.</p>

        <p class="block">"Data is stored persistently in an attached PostgreSQL database.
        The application itsself is written in "<a href="https://www.rust-lang.org/" target="_blank"> rust</a>.</p>

        <p class="block">"For more information, please contact "
            <a href="mailto:mwb@quantlink.de?Subject=Quinvestor">the author</a>.
        </p>
        </div>
    </div>
    }
}

#[component]
fn Transactions() -> impl IntoView {
    use crate::transactions;

    view! {
        <div class="center">
            <h1>Transactions</h1>
            <Await future=transactions::get_transactions(
                transactions::TransactionFilter {
                   user_id: 1,
                })
            let:transactions
            >
               <TransactionTable transactions={transactions.as_ref().unwrap().get()}/>
            </Await>
        </div>
    }
}

#[component]
pub fn TransactionEditButtonRenderer(
    row: RwSignal<TransactionView>,
    index: usize,
) -> impl IntoView {
    let reload_controller = use_context::<ReloadController>().expect("ReloadController not found");
    let delete_row_set =
        use_context::<WriteSignal<Option<i32>>>().expect("delete row signal not found");

    let on_click = move |_| match index {
        1 => match row.get().state {
            TransactionDisplay::View => {
                row.update(|row| row.state = TransactionDisplay::Edit);
                reload_controller.reload();
            }
            TransactionDisplay::Edit => {
                row.update(|row| row.state = TransactionDisplay::View);
                reload_controller.reload();
            }
        },
        2 => {
            delete_row_set.set(Some(row.read().id));
        }
        _ => unreachable!(),
    };

    view! {
        <td>
            <img class="icon" width=25 src={move || row.read().state.get_icon(index)} on:click=on_click />
        </td>
    }
}

impl TableDataProvider<TransactionView> for RwSignal<Vec<TransactionView>> {
    async fn get_rows(
        &self,
        _: Range<usize>,
    ) -> Result<(Vec<TransactionView>, Range<usize>), String> {
        let transactions = self.read().clone();
        let len = transactions.len();
        Ok((transactions, 0..len))
    }

    async fn row_count(&self) -> Option<usize> {
        Some(self.get_untracked().len())
    }
}

#[allow(non_snake_case)]
pub fn TransactionRowRenderer(
    class: Signal<String>,
    row: RwSignal<TransactionView>,
    _index: usize,
    _selected: Signal<bool>,
    _on_select: EventHandler<web_sys::MouseEvent>,
) -> impl IntoView {
    match row.read().state {
        TransactionDisplay::View => view! {
            <tr class=class>
                <td>{move || row.read().group_id}</td>
                <td>{move || row.read().asset_name.clone()}</td>
                <td>{move || row.read().position}</td>
                <td>{move || row.read().trans_type.clone()}</td>
                <td>{move || row.read().cash_amount}</td>
                <td>{move || row.read().cash_currency.clone()}</td>
                <td>{move || row.read().cash_date.clone()}</td>
                <td>{move || row.read().note.clone()}</td>
                <TransactionEditButtonRenderer row index=1 />
                <TransactionEditButtonRenderer row index=2 />
            </tr>
        }
        .into_any(),
        TransactionDisplay::Edit => view! {
            <tr class=class>
                <td>{move || row.read().group_id}</td>
                <td>{move || row.read().asset_name.clone()}</td>
                <td>{move || row.read().position}</td>
                <td>{move || row.read().trans_type.clone()}</td>
                <td>{move || row.read().cash_amount}</td>
                <td>{move || row.read().cash_currency.clone()}</td>
                <td>{move || row.read().cash_date.clone()}</td>
                <td>{move || row.read().note.clone()}</td>
                <TransactionEditButtonRenderer row index=1 />
                <TransactionEditButtonRenderer row index=2 />
            </tr>
        }
        .into_any(),
    }
}

#[component]
fn TransactionTable(transactions: Vec<TransactionView>) -> impl IntoView {
    let rows = RwSignal::new(transactions);

    let on_change = move |evt: ChangeEvent<TransactionView>| {
        rows.write()[evt.row_index] = evt.changed_row.get_untracked();
    };

    let reload_controller = ReloadController::default();
    let add_row = move |_ev: MouseEvent| {
        let mut min_id = 0;
        for row in rows.read().iter() {
            min_id = min_id.min(row.id);
        }
        let new_row = TransactionView::new(min_id - 1);
        rows.write().push(new_row);
        reload_controller.reload();
    };
    let (delete_row, delete_row_set) = signal::<Option<i32>>(None);

    provide_context(reload_controller);
    provide_context(delete_row_set);

    Effect::new(move || {
        if let Some(row_id) = delete_row.get() {
            let mut index = None;
            for (i, row) in rows.read().iter().enumerate() {
                if row.id == row_id {
                    index = Some(i);
                    break;
                }
            }
            if let Some(index) = index {
                rows.write().remove(index);
                reload_controller.reload();
            }
        }
    });

    view! {
        <img class="icon" width=25 src="plus.svg" on:click=add_row />
        <table class="transactions">
            <TableContent rows row_renderer=TransactionRowRenderer on_change scroll_container="html" />
        </table>
    }
}

#[component]
fn Position() -> impl IntoView {
    view! {
        <div class="center">
            <h1>Position</h1>
            <p>Here you can see your positions.</p>
        </div>
    }
}

#[derive(TableRow, Clone)]
#[table(impl_vec_data_provider)]
pub struct Person {
    id: u32,
    name: String,
    age: u32,
}

#[component]
fn Assets() -> impl IntoView {
    let rows = vec![
        Person {
            id: 1,
            name: "John".to_string(),
            age: 32,
        },
        Person {
            id: 2,
            name: "Jane".to_string(),
            age: 28,
        },
        Person {
            id: 3,
            name: "Bob".to_string(),
            age: 45,
        },
    ];

    view! {
        <div class="center">
            <h1>Assets</h1>
            <table>
                <TableContent rows scroll_container="html" />
            </table>
       </div>
    }
}

#[component]
fn Performance() -> impl IntoView {
    view! {
        <div class="center">
            <h1>Performance</h1>
            <p>Here you can see your performance.</p>
        </div>
    }
}
#[component]
fn Settings() -> impl IntoView {
    view! {
        <div class="center">
            <h1>Settings</h1>
            <p>Here you can see your settings.</p>
        </div>
    }
}
#[component]
fn Accounts() -> impl IntoView {
    view! {
        <div class="center">
            <h1>Accounts</h1>
            <p>Here you can see your accounts.</p>
        </div>
    }
}
#[component]
fn Nav() -> impl IntoView {
    let nav_menu = RwSignal::new(false);

    view! {
        <header id="top" class="w3-container">
        <div class="topnav">
            <nav class="nav">
                <ul>
                    <li class="logo"><A href="/">QuantInvest</A></li>
                    <li class={move || if nav_menu.get() { "show" } else { "" } }><A href="/transactions">Transactions</A></li>
                    <li class={move || if nav_menu.get() { "show" } else { "" } }><A href="/position">Position</A></li>
                    <li class={move || if nav_menu.get() { "show" } else { "" } }><A href="/assets">Assets</A></li>
                    <li class={move || if nav_menu.get() { "show" } else { "" } }><A href="/performance">Performance</A></li>
                    <li class={move || if nav_menu.get() { "show" } else { "" } }><A href="/settings">Settings</A></li>
                    <li class={move || if nav_menu.get() { "show" } else { "" } }><A href="/accounts">Accounts</A></li>
                </ul>
                //<button id="hamburger" on:click=move |_| nav_menu.update(|f| *f=!(*f)) />
            </nav>
        </div>
        </header>
    }
}
