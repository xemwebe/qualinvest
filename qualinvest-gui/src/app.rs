use crate::auth::User;
use crate::transaction_view::TransactionsTable;
use leptos::prelude::*;
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
                <Routes fallback=|| "Page not found.".into_view()>
                    <Route path=StaticSegment("") view=HomePage/>
                    <Route path=StaticSegment("login") view=Login/>
                    <Route path=StaticSegment("transactions") view=|| { view!{ <ProtectedRoute><Transactions/></ProtectedRoute> } }/>
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
    let login_action = Action::new(|input: &(String, String)| {
        let username = input.0.clone();
        let password = input.1.clone();
        async move { login_user(username, password).await }
    });

    let (username, set_username) = signal(String::new());
    let (password, set_password) = signal(String::new());

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
                            use leptos_router::hooks::use_navigate;
                            let navigate = use_navigate();
                            navigate("/", Default::default());
                            view! { <p>"Login successful!"</p> }.into_any()
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
    let user = expect_context::<Resource<Option<User>>>();
    debug!("ProtectedRoute user: {:?}", user);
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
               <TransactionsTable transactions={transactions.as_ref().unwrap().get()}/>
            </Await>
        </div>
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
fn Assets() -> impl IntoView {
    view! {
        <div class="center">
            <h1>Assets</h1>
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

    let logout_action = Action::new(|_: &()| async move { logout_user().await });

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
                                            <button on:click=move |_| { logout_action.dispatch(()); }>
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
