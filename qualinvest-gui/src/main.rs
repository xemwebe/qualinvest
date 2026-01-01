use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(feature = "ssr")] {

        use finql::{
            postgres::PostgresDB,
            datatypes::ObjectHandler,
            market::Market,
            market_quotes::MarketDataSource,
        };
        use leptos::prelude::LeptosOptions;
        use qualinvest_gui::app::*;
        use qualinvest_gui::auth::{PostgresBackend};
        use axum_login::{AuthSession};

        use anyhow::Result;
        use axum::{
            body::Body as AxumBody,
            extract::{FromRef, Path, RawQuery, State},
            response::{IntoResponse, Response as AxumResponse},
            routing::{get, post},
            Router,
            http::{HeaderMap, Request, StatusCode, Uri},
        };
        use clap::Parser;
        //use http::{HeaderMap, Request};
        use leptos::prelude::*;
        use leptos_axum::{generate_route_list, handle_server_fns_with_context, LeptosRoutes};
        use log::{debug, info, error};
        use serde::{Deserialize, Serialize};
        use std::path::PathBuf;
        use std::net::{IpAddr, Ipv4Addr, SocketAddr};
        use qualinvest_gui::{
            error_template::{AppError, ErrorTemplate},
            global_settings::GlobalSettings,
        };
        use tower::ServiceExt;
        use tower_http::services::ServeDir;
        use axum_login::AuthManagerLayerBuilder;
        use tower_sessions::{SessionManagerLayer, cookie::{Key, SameSite}, Expiry, ExpiredDeletion};
        use tokio::{signal, task::AbortHandle};
        use tower_sessions_sqlx_store::PostgresStore;
        use time::Duration;

        pub async fn file_and_error_handler(
            uri: Uri,
            State(options): State<LeptosOptions>,
            req: Request<AxumBody>,
        ) -> AxumResponse {
            debug!("File and error handler called");
            let root = options.site_root.clone();
            let res = get_static_file(uri.clone(), &root).await.unwrap();

            if res.status() == StatusCode::OK {
                res.into_response()
            } else {
                let mut errors = Errors::default();
                errors.insert_with_default_key(AppError::NotFound);
                let handler = leptos_axum::render_app_to_stream(
                    move || view! { <ErrorTemplate outside_errors=errors.clone()/>},
                );
                handler(req).await.into_response()
            }
        }

        async fn get_static_file(uri: Uri, root: &str) -> Result<AxumResponse<AxumBody>, (StatusCode, String)> {
            let req = Request::builder()
                .uri(uri.clone())
                .body(AxumBody::empty())
                .unwrap();
            // `ServeDir` implements `tower::Service` so we can call it with `tower::ServiceExt::oneshot`
            // This path is relative to the cargo root
            debug!("Serving file {:?}", req);
            match ServeDir::new(root).oneshot(req).await {
                Ok(res) => Ok(res.into_response()),
                Err(err) => {
                    error!("Error serving file: {}", err);
                    Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Something went wrong: {err}"),
                ))},
            }
        }

        async fn leptos_routes_handler(
            State(app_state): State<AppState>,
            auth_session: AuthSession<PostgresBackend>,
            req: Request<AxumBody>,
        ) -> AxumResponse {
            let handler = leptos_axum::render_app_to_stream_with_context(
                move || {
                    provide_context(auth_session.clone());
                },
                move || shell(app_state.leptos_options.clone()),
            );
            handler(req).await.into_response()
        }

        #[derive(Clone, FromRef)]
        pub struct AppState {
            pub db: PostgresDB,
            pub market: Market,
            pub leptos_options: LeptosOptions,
            pub global_settings: GlobalSettings,
        }

        #[derive(Parser)]
        #[command(author, version, about, long_about = None)]
        struct Cli {
            #[arg(short, long, value_name = "FILE")]
            config: Option<PathBuf>,
        }

        #[derive(Default, Debug, Serialize, Deserialize)]
        pub struct Configuration {
            pub port: u16,
            pub database_url: String,
        }

        async fn server_fn_handler(
            State(app_state): State<AppState>,
            auth_session: AuthSession<PostgresBackend>,
            path: Path<String>,
            _headers: HeaderMap,
            _raw_query: RawQuery,
            request: Request<AxumBody>,
        ) -> impl IntoResponse {
            info!("{:?}", path);

            handle_server_fns_with_context(
                move || {
                    provide_context(app_state.db.clone());
                    provide_context(app_state.market.clone());
                    provide_context(app_state.global_settings.clone());
                    provide_context(auth_session.clone());
                },
                request,
            )
            .await
        }

        async fn create_market(db: &PostgresDB, inception_date: time::Date) -> Result<Market> {
            let db = std::sync::Arc::new(db.clone());
            let end_date = time::Date::from_calendar_date(inception_date.year() + 100, inception_date.month(), inception_date.day())?;
            let market = Market::new_with_date_range(db, inception_date.into(), end_date).await?;
            let yahoo = MarketDataSource::Yahoo;
            market.add_provider(
                yahoo.to_string(),
                yahoo.get_provider(String::new()).unwrap(),
            );
            info!("Market created.");
            Ok(market)
        }

        #[tokio::main]
        async fn main() -> Result<()> {
            simple_logger::init_with_level(log::Level::Debug)?;
            let cli = Cli::parse();
                        let config: Configuration = if let Some(config_path) = cli.config.as_deref() {
                            confy::load_path(config_path)?
                        } else {
                            debug!(
                                "Reading default configuration file {}",
                                confy::get_configuration_file_path("qualinvest", None)?.display()
                            );
                            confy::load("qualinvest", None)?
                        };

                       let db = PostgresDB::new(&config.database_url)
                            .await
                            .expect("failed to open database");
                        let mut leptos_options = get_configuration(None)
                            .expect("failed to load leptos options")
                            .leptos_options;
                        let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), config.port);
                        leptos_options.site_addr = socket;

                        // get global settings from database
                        let global_settings = if let Ok(global_settings) = db.get_object("global_settings").await {
                            global_settings
                        } else {
                            let global_settings = GlobalSettings::default();
                            db.store_object("global_settings", &global_settings).await?;
                            global_settings
                        };
                        info!("Global settings loaded: {:?}", global_settings);

                        let market = create_market(&db, global_settings.inception_date.date()).await?;

                        // Session layer
                        //
                        // This uses `tower-sessions`to establish a layer that will provide the
                        // session as a request service
                        let session_store = PostgresStore::new(db.pool.clone());
                        session_store.migrate().await?;

                        let deletion_task = tokio::task::spawn(
                            session_store.clone().continuously_delete_expired(tokio::time::Duration::from_secs(600)),
                        );

                        // Generate a cryptographic key for session management
                        // Note: Currently generates a random key on each startup. This means sessions
                        // will be invalidated when the server restarts, which is acceptable for
                        // development but may need to be loaded from config for production persistence.
                        let key = Key::generate();
                        // Set up session management with secure cookie settings
                        let session_layer = SessionManagerLayer::new(session_store)
                            .with_secure(true)  // Only send cookies over HTTPS
                            .with_http_only(true)  // Prevent JavaScript access to cookies
                            .with_same_site(SameSite::Strict)  // CSRF protection
                            .with_expiry(Expiry::OnInactivity(Duration::minutes(10)))
                            .with_signed(key);

                        // Auth service
                        //
                        // This combines the session layer with our backend to establish the auth
                        // service which will provide the auth session as a request extension
                        let backend = PostgresBackend::new(db.pool.clone());
                        let auth_layer = AuthManagerLayerBuilder::new(backend, session_layer).build();

                        let app_state = AppState {
                            db,
                            market,
                            leptos_options,
                            global_settings,
                        };

                        let routes = generate_route_list(|| view! { <App/> });
                        // build our application with a route
                        let app = Router::new()
                            .nest_service("/public", ServeDir::new("public"))
                            .route("/api/*fn_name", post(server_fn_handler))
                            .leptos_routes_with_handler(routes, get(leptos_routes_handler))
                            .fallback(file_and_error_handler)
                            .layer(auth_layer)
                            .with_state(app_state);

            // run our app with hyper
            // `axum::Server` is a re-export of `hyper::Server`
            info!("listening on http://{}", &socket);
            let listener = tokio::net::TcpListener::bind(&socket).await.unwrap();
            axum::serve(listener, app.into_make_service())
                .with_graceful_shutdown(shutdown_signal(deletion_task.abort_handle()))
                .await
                .unwrap();
            Ok(())
        }

        async fn shutdown_signal(deletion_task_abort_handle: AbortHandle) {
            let ctrl_c = async {
                signal::ctrl_c()
                    .await
                    .expect("failed to install Ctrl+C handler");
            };

            #[cfg(unix)]
            let terminate = async {
                signal::unix::signal(signal::unix::SignalKind::terminate())
                    .expect("failed to install signal handler")
                    .recv()
                    .await;
            };

            #[cfg(not(unix))]
            let terminate = std::future::pending::<()>();

            tokio::select! {
                _ = ctrl_c => { deletion_task_abort_handle.abort() },
                _ = terminate => { deletion_task_abort_handle.abort() },
            }
        }

    } else {
        pub fn main() {
            // no client-side main function
        }
    }
}
