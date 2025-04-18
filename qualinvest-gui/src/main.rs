use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(feature = "ssr")] {

        use finql::postgres::PostgresDB;
        use leptos::prelude::LeptosOptions;
        use qualinvest_gui::app::*;

        use anyhow::Result;
        use axum::{
            body::Body as AxumBody,
            extract::{FromRef, Path, RawQuery, State},
            response::{IntoResponse, Response as AxumResponse},
            routing::{get, post},
            Router,
            http::{HeaderMap, Request, Response, StatusCode, Uri},
        };
        use clap::Parser;
        //use http::{HeaderMap, Request};
        use leptos::prelude::*;
        use leptos_axum::{generate_route_list, handle_server_fns_with_context, LeptosRoutes};
        use log::{debug, info, error};
        use serde::{Deserialize, Serialize};
        use std::path::PathBuf;
        use std::net::{IpAddr, Ipv4Addr, SocketAddr};
        use qualinvest_gui::error_template::{AppError, ErrorTemplate};
        use leptos::prelude::*;
        use tower::ServiceExt;
        use tower_http::services::ServeDir;

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
            req: Request<AxumBody>,
        ) -> AxumResponse {
            let handler = leptos_axum::render_app_to_stream_with_context(
                move || {
                    provide_context(app_state.db.clone());
                },
                move || shell(app_state.leptos_options.clone()),
            );
            handler(req).await.into_response()
        }

        #[derive(Clone, FromRef)]
        pub struct AppState {
            pub db: PostgresDB,
            pub leptos_options: LeptosOptions,
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
            path: Path<String>,
            _headers: HeaderMap,
            _raw_query: RawQuery,
            request: Request<AxumBody>,
        ) -> impl IntoResponse {
            info!("{:?}", path);

            handle_server_fns_with_context(
                move || {
                    provide_context(app_state.db.clone());
                },
                request,
            )
            .await
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

                       let db =PostgresDB::new(&config.database_url)
                            .await
                            .expect("failed to open database");
                        let mut leptos_options = get_configuration(None)
                            .expect("failed to load leptos options")
                            .leptos_options;
                        let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), config.port);
                        leptos_options.site_addr = socket;

                        let app_state = AppState {
                           db,
                            leptos_options,
                        };

                        let routes = generate_route_list(|| view! { <App/> });
                        // build our application with a route
                        let app = Router::new()
                            .nest_service("/public", ServeDir::new("public"))
                            .route("/api/*fn_name", post(server_fn_handler))
                            .leptos_routes_with_handler(routes, get(leptos_routes_handler))
                            .fallback(file_and_error_handler)
                            .with_state(app_state);

            // run our app with hyper
            // `axum::Server` is a re-export of `hyper::Server`
            info!("listening on http://{}", &socket);
            let listener = tokio::net::TcpListener::bind(&socket).await.unwrap();
            axum::serve(listener, app.into_make_service())
                .await
                .unwrap();
            Ok(())
        }
    } else {
        pub fn main() {
            // no client-side main function
        }
    }
}
