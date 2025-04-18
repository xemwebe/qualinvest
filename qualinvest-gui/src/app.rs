use crate::transactions;
use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes, A},
    StaticSegment,
};

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
fn TransactionTable(transactions: Vec<transactions::TransactionView>) -> impl IntoView {
    let rows = transactions
        .into_iter()
        .map(|transaction| {
            view! {
            <tr>
                <td>{transaction.group_id}</td>
                <td>{transaction.asset_name}</td>
                <td>{transaction.position}</td>
                <td>{transaction.trans_type}</td>
                <td>{transaction.cash_amount}</td>
                <td>{transaction.cash_currency}</td>
                <td>{transaction.cash_date}</td>
                <td>{transaction.doc_path}</td>
                <td>{transaction.note}</td>
            </tr> }
        })
        .collect_view();

    view! {
        <table>
            <tr>
                <th>"Group ID"</th>
                <th>"Asset Name"</th>
                <th>"Position"</th>
                <th>"Type"</th>
                <th>"Amount"</th>
                <th>"Currency"</th>
                <th>"Date"</th>
                <th>"Documentation"</th>
                <th>"Note"</th>
            </tr>
            {rows}
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
                </ul>
                //<button id="hamburger" on:click=move |_| nav_menu.update(|f| *f=!(*f)) />
            </nav>
        </div>
        </header>
    }
}
