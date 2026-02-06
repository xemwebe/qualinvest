use crate::auth::User;
use crate::position_view::PositionTable;
use crate::quote_graph::QuotesGraph;
use crate::transaction_view::TransactionsTable;
use leptos::{prelude::*, task::spawn_local};
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes, A},
    StaticSegment,
};
use log::debug;

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

    let user = Resource::new(|| {}, |_| async move { get_user().await.ok().flatten() });
    provide_context(user);

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
                <Routes fallback=|| {
                    debug!("Route fallback triggered - page not found");
                    "Page not found.".into_view()
                }>
                    <Route path=StaticSegment("") view=HomePage/>
                    <Route path=StaticSegment("login") view=Login/>
                    <Route path=StaticSegment("transactions") view=|| {
                        debug!("Transactions route matched");
                        view!{ <ProtectedRoute><Transactions/></ProtectedRoute> }
                    }/>
                    <Route path=StaticSegment("position") view=|| { view!{ <ProtectedRoute><Position/></ProtectedRoute> } }/>
                    <Route path=StaticSegment("assets") view=|| { view!{ <ProtectedRoute><Assets/></ProtectedRoute> } }/>
                    <Route path=StaticSegment("performance") view=|| { view!{ <ProtectedRoute><Performance/></ProtectedRoute> } }/>
                    <Route path=StaticSegment("settings") view=|| { view!{ <ProtectedRoute><Settings/></ProtectedRoute> } }/>
                    <Route path=StaticSegment("accounts") view=|| { view!{ <ProtectedRoute><Accounts/></ProtectedRoute> } }/>
                </Routes>
            </main>
        </Router>
    }
}

/// Renders the home page of your application.
#[component]
fn HomePage() -> impl IntoView {
    let user = expect_context::<Resource<Option<User>>>();

    view! {
        <div class="center">
            <Suspense fallback=|| view! { <p>"Loading..."</p> }>
                {move || {
                    user.get().map(|user_data| {
                        match user_data {
                            Some(user) => view! {
                                <div class="warning,block">
                                    "You are logged in as " {user.name.clone()}
                                </div>
                            }.into_any(),
                            None => view! {
                                <div class="warning">
                                    <h1>"Please log in!"</h1>
                                    <A href="/login">"Login"</A>
                                </div>
                            }.into_any(),
                        }
                    })
                }}
            </Suspense>

            <h1>"Quant Invest"</h1>
            <div class="inline">
                <p class="block">"Quant Invest is a tool to manage a portfolio of investments of common assets like shares, bonds or loans."</p>
                <p class="block">"The functionality covers basic book-keeping of positions, paid fees and tax and calculation of a couple of performance figures, eg. realised and unrealised p&l over specific time periods. Market data is automatically retreived from various, configurable sources."</p>
                <p class="block">"Data is stored persistently in an attached PostgreSQL database. The application itsself is written in " <a href="https://www.rust-lang.org/" target="_blank">"rust"</a>"."</p>
                <p class="block">"For more information, please contact " <a href="mailto:mwb@quantlink.de?Subject=Quinvestor">"the author"</a>"."</p>
            </div>
        </div>
    }
}

#[server(LoginUser, "/api")]
pub async fn login_user(username: String, password: String) -> Result<(), ServerFnError> {
    use crate::auth::{Credentials, PostgresBackend};
    use axum_login::AuthSession;
    use log::debug;

    debug!("Logging in user");
    let mut auth: AuthSession<PostgresBackend> = expect_context();
    let credentials = Credentials { username, password };

    match auth.authenticate(credentials).await {
        Ok(Some(user)) => {
            let session = auth.login(&user).await;
            if session.is_err() {
                debug!("failed to establish session error: {session:?}");
                Err(ServerFnError::new("Failed to establish session"))
            } else {
                Ok(())
            }
        }
        Ok(None) => {
            debug!("invalid credentilas");
            Err(ServerFnError::new("Invalid credentials"))
        }
        Err(e) => {
            debug!("failed to verify credentials: {e}");
            Err(ServerFnError::new("Failed to verify credentials"))
        }
    }
}

#[server(LogoutUser, "/api")]
pub async fn logout_user() -> Result<(), ServerFnError> {
    use crate::auth::PostgresBackend;
    use axum_login::AuthSession;
    debug!("logging out user");
    let mut auth: AuthSession<PostgresBackend> = expect_context();
    let _ = auth.logout().await;
    Ok(())
}

#[server(GetUser, "/api")]
pub async fn get_user() -> Result<Option<User>, ServerFnError> {
    use crate::auth::PostgresBackend;
    use axum_login::AuthSession;
    let auth: AuthSession<PostgresBackend> = expect_context();
    debug!("got user: {:?}", auth.user);
    Ok(auth.user.clone())
}

#[component]
fn Login() -> impl IntoView {
    let user = expect_context::<Resource<Option<User>>>();

    let login_action = Action::new(|input: &(String, String)| {
        let username = input.0.clone();
        let password = input.1.clone();
        async move { login_user(username, password).await }
    });

    let (username, set_username) = signal(String::new());
    let (password, set_password) = signal(String::new());
    let (should_navigate, set_should_navigate) = signal(false);
    let username_input_ref = NodeRef::<leptos::html::Input>::new();

    // Set focus on username input when component mounts
    Effect::new(move |_| {
        if let Some(input) = username_input_ref.get() {
            let _ = input.focus();
        }
    });

    // Effect to handle navigation after successful login and user refetch
    Effect::new(move |_| {
        if should_navigate.get() {
            if let Some(user_data) = user.get() {
                if user_data.is_some() {
                    debug!("User is authenticated, navigating to /transactions");
                    use leptos_router::hooks::use_navigate;
                    let navigate = use_navigate();
                    navigate("/transactions", Default::default());
                    set_should_navigate.set(false);
                }
            }
        }
    });

    view! {
        <div class="center">
            <h1>"Login"</h1>
            <form on:submit=move |ev| {
                ev.prevent_default();
                login_action.dispatch((username.get(), password.get()));
            }>
                <div class="form-group">
                    <label for="username">"Username:"</label>
                    <input
                        type="text"
                        id="username"
                        name="username"
                        node_ref=username_input_ref
                        prop:value=username
                        on:input=move |ev| set_username.set(event_target_value(&ev))
                        required
                    />
                </div>
                <div class="form-group">
                    <label for="password">"Password:"</label>
                    <input
                        type="password"
                        id="password"
                        name="password"
                        prop:value=password
                        on:input=move |ev| set_password.set(event_target_value(&ev))
                        required
                    />
                </div>
                <button type="submit" disabled=move || login_action.pending().get()>
                    {move || if login_action.pending().get() { "Logging in..." } else { "Login" }}
                </button>
            </form>
            {move || {
                login_action.value().get().map(|result| {
                    match result {
                        Ok(_) => {
                            // Refetch user data after successful login
                            debug!("Login successful, refetching user data");
                            user.refetch();
                            set_should_navigate.set(true);
                            view! { <p>"Login successful! Redirecting..."</p> }.into_any()
                        },
                        Err(err) => view! { <p class="error">"Login failed: " {err.to_string()}</p> }.into_any(),
                    }
                })
            }}
        </div>
    }
}

#[component]
fn ProtectedRoute(children: ChildrenFn) -> impl IntoView {
    debug!("ProtectedRoute component called");
    let user = expect_context::<Resource<Option<User>>>();
    debug!("ProtectedRoute user resource: {:?}", user);
    view! {
        <Suspense fallback=|| view! { <p>"Loading..."</p> }>
            {move || {
                debug!("ProtectedRoute: trying to get user");
                user.get().map(|user_data| {
                    debug!("ProtectedRoute: user data: {:?}", user_data);
                    match user_data {
                        Some(_) => children().into_any(),
                        None => {
                            use leptos_router::hooks::use_navigate;
                            let navigate = use_navigate();
                            navigate("/login", Default::default());
                            view! { <p>"Redirecting to login..."</p> }.into_any()
                        }
                    }
                })
            }}
        </Suspense>
    }
}

#[component]
fn Transactions() -> impl IntoView {
    let user = expect_context::<Resource<Option<User>>>();
    let (selected_account_id, set_selected_account_id) = signal::<Option<i32>>(None);

    view! {
        <div class="center">
            <h1>Transactions</h1>
            <Suspense fallback=|| view! { <p>"Loading..."</p> }>
                {move || {
                    user.get().and_then(|user_data| {
                        user_data.map(|u| {
                            view! {
                                <TransactionsTable
                                    user_id=u.id
                                    selected_account_id=selected_account_id
                                    set_selected_account_id=set_selected_account_id
                                />
                            }
                        })
                    })
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn Position() -> impl IntoView {
    view! {
        <div class="center">
            <h1>Position</h1>
            <PositionTable />
        </div>
    }
}

#[component]
fn Assets() -> impl IntoView {
    use crate::asset_view::AssetsTable;
    use crate::assets;
    let (selected_asset_info, set_selected_asset_info) = signal::<Option<(i32, String)>>(None);
    let (selected_ticker_info, set_selected_ticker_info) = signal::<Option<(i32, String)>>(None);

    view! {
        <div class="center" id="assetpage">
            <div id="asset-header">
                <h1>Assets</h1>
            </div>
            <div id="assets">
            <h2>"Asset List"</h2>
            <Suspense fallback=|| view! { <p>"Loading..."</p> }>
                {move ||
                    view! {
                        <Await future=assets::get_assets()
                        let:assets
                        >
                           <AssetsTable
                               assets={assets.as_ref().unwrap().get()}
                               selected_asset_info=selected_asset_info
                               set_selected_asset_info=set_selected_asset_info
                               set_selected_ticker_info=set_selected_ticker_info
                           />
                        </Await>
                    }
                }
            </Suspense>
                {move || {
                    selected_asset_info.get().map(|(asset_id, asset_name)| {
                        view!{
                            <Tickers asset_id=asset_id asset_name=asset_name selected_ticker_info=selected_ticker_info set_selected_ticker_info=set_selected_ticker_info selected_asset_info=selected_asset_info />
                        }
                    })
                }}
            </div>
            <div id="quote-graph">
                {move || {
                    selected_ticker_info.get().map(|(ticker_id, ticker_name)| {
                        view!{
                            <h3>"Price History Graph for " {ticker_name}</h3>
                            <QuotesGraph ticker_id=ticker_id />
                        }
                    })
                }}
            </div>
            <div id="quote-table">
                {move || {
                    selected_ticker_info.get().map(|(ticker_id, ticker_name)| {
                        view!{
                            <h3>"Quote Data Table for ticker" {ticker_name}</h3>
                            <Quotes ticker_id=ticker_id />
                        }
                    })
                }}
            </div>
        </div>
    }
}

#[component]
fn Tickers(
    asset_id: i32,
    asset_name: String,
    selected_ticker_info: ReadSignal<Option<(i32, String)>>,
    set_selected_ticker_info: WriteSignal<Option<(i32, String)>>,
    selected_asset_info: ReadSignal<Option<(i32, String)>>,
) -> impl IntoView {
    use crate::ticker;
    use crate::ticker_view::TickersTable;

    view! {
        <div id="ticker-table">
        <h2>"Tickers for Asset " {asset_name}</h2>
        <Suspense fallback=|| view! { <p>"Loading tickers..."</p> }>
            <Await future=ticker::get_tickers(
                ticker::TickerFilter { asset_id }
            )
            let:ticker
            >
                <TickersTable
                    tickers={ticker.as_ref().unwrap().get()}
                    selected_ticker_info=selected_ticker_info
                    set_selected_ticker_info=set_selected_ticker_info
                    selected_asset_info=selected_asset_info
                />
            </Await>
        </Suspense>
        </div>
    }
}

#[component]
fn Quotes(ticker_id: i32) -> impl IntoView {
    use crate::quote_view::QuotesTable;
    use crate::quotes;

    view! {
        <div class="quote-table">
        <Suspense fallback=|| view! { <p>"Loading quotes..."</p> }>
            <Await future=quotes::get_quotes(
                quotes::QuoteFilter { ticker_id }
            )
            let:quotes
            >
                <QuotesTable quotes={quotes.as_ref().unwrap().get()}/>
            </Await>
        </Suspense>
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
    let user = expect_context::<Resource<Option<User>>>();

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
                    <Suspense fallback=|| view! { <li></li> }>
                        {move || {
                            user.get().map(|user_data| {
                                match user_data {
                                    Some(_) => view! {
                                        <li class={move || if nav_menu.get() { "show" } else { "" } }>
                                            <button on:click=move |_| {
                                                spawn_local(async {
                                                    let _ = logout_user().await;
                                                    let _ = window().location().set_href("/");
                                                });
                                            }>
                                                "Logout"
                                            </button>
                                        </li>
                                    }.into_any(),
                                    None => view! {
                                        <li class={move || if nav_menu.get() { "show" } else { "" } }>
                                            <A href="/login">"Login"</A>
                                        </li>
                                    }.into_any(),
                                }
                            })
                        }}
                    </Suspense>
                </ul>
            </nav>
        </div>
        </header>
    }
}
