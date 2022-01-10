//! This library is part of a set of tools for quantitative investments.
//! For mor information, see [qualinvest on github](https://github.com/xemwebe/qualinvest)
//!
//! Once you have set-up a fresh database (or cleaned an existing database), you need to manually add
//! a new admin to start working with the empty database. Login in into the database with `psql`,
//! connect to your database, and add a new user with the following SQL query (make sure to choose 
//! a proper password!)
//!
//! ```SQL
//! INSERT INTO users (name, display, salt_hash, is_admin) 
//! VALUES ('admin', 'Admin', crypt('admin123',gen_salt('bf',8)), TRUE);
//! ```

#[macro_use] extern crate rocket;
#[macro_use] extern crate serde;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use clap::{App, AppSettings, Arg};

use rocket::{State};
use rocket::http::{uri::Origin, Cookie, CookieJar};
use rocket::fs::NamedFile;
use rocket::response::{Redirect, Flash};
use rocket::request::{FlashMessage};
use rocket::form::Form;
use rocket_dyn_templates::Template;
use rocket::figment::Figment;

use finql_postgres::PostgresDB;
use qualinvest_core::{Config, MarketDataProviders};

mod accounts;
mod asset;
mod position;
mod transactions;
mod helper;
mod filter;
mod auth;
mod user;
mod layout;
mod form_types;
mod ticker;
mod quotes;
mod user_settings;

use auth::authorization::*;
use user::*;
use layout::*;
use once_cell::sync::OnceCell;

static BASE_PATH: OnceCell<String> = OnceCell::new();

pub struct ServerState {
    postgres_db: Arc<PostgresDB>,
    doc_path: String,
    market_data: MarketDataProviders,
}

impl ServerState {
    pub fn default_context(&self) -> tera::Context {
        let mut context = tera::Context::new();
        let mut rel_path = Self::base().to_string();
        if !rel_path.ends_with('/') {
            rel_path.push('/');
        }
        context.insert("relpath", &rel_path);
        context
    }

    pub fn set_base(base: String) {
        let _ = BASE_PATH.set(base);
    }

    pub fn base() -> Origin<'static> {
        Origin::parse(BASE_PATH.get().unwrap()).expect("Invalid base path")
    }
}

/// The `logged_in()` method queries the database for the username specified
/// in the cookie.  In this instance all of the data in the database is also
/// contained in the cookie, making a database operation unnecessary, however
/// this is just an example to show how to connect to a database.
#[get("/login", rank = 1)]
async fn logged_in(user: UserCookie, state: &State<ServerState>) -> Template {
    let mut context = state.default_context();
    context.insert("user", &user);
    layout("index", &context.into_json())
}

#[get("/login?<redirect>", rank = 2)]
async fn login(redirect: Option<String>, state: &State<ServerState>) -> Template {
    let mut context = state.default_context();
    if let Some(redirect) = redirect {
        context.insert("redirect", &redirect);
    }
    layout("login", &context.into_json())
}

/// if there is a user query string, and an optional flash message
/// display an optional flash message indicating why the login failed
/// and the login screen with user filled in
#[get("/login?<user>&<redirect>")]
async fn retry_login_user(user: UserQuery, redirect: Option<String>, flash_msg_opt: Option<FlashMessage<'_>>, state: &State<ServerState>) -> Template {
    let mut context = state.default_context();
    if let Some(flash) = flash_msg_opt {
        context.insert("alert_type", &flash.kind());
        context.insert("alert_msg", &flash.message());
    }
    context.insert("user", &user.user);
    if let Some(redirect) = redirect {
        context.insert("redirect", &redirect);
    }
    layout("login", &context.into_json())
}

/// if there is a flash message but no user query string
/// display why the login failed and display the login screen
#[get("/login?<redirect>&<err_msg>", rank = 3)]
async fn retry_login_flash(redirect: Option<String>, err_msg: Option<String>, state: &State<ServerState>) -> Template {
    let mut context = state.default_context();

    if let Some(redirect) = redirect {
        context.insert("redirect", &redirect);
    }
    context.insert("err_msg", &err_msg);
    layout("login", &context.into_json())
}

#[post("/login", data = "<form>")]
async fn process_login(form: Form<UserForm>, cookies: &CookieJar<'_>, 
        state: &State<ServerState>) -> Result<Redirect, Flash<Redirect>> {
    let db = state.postgres_db.clone();
    let login = form.into_inner();
    login.flash_redirect(login.redirect.clone(), format!("/{}login", ServerState::base()), cookies, db).await
}

#[get("/logout")]
async fn logout(user: Option<UserCookie>, cookies: &CookieJar<'_>) -> Result<Redirect, Redirect> {
    if user.is_some() {
        cookies.remove_private(Cookie::named(UserCookie::cookie_id()));
        Ok(Redirect::to(uri!(ServerState::base(), index(Some("Successfully logged out".to_string())))))
    } else {
        Err(Redirect::to(uri!(ServerState::base(), index(Option::<String>::None))))
    }
}

#[get("/?<message>")]
async fn index(message: Option<String>, user_opt: Option<UserCookie>, flash_msg_opt: Option<FlashMessage<'_>>, state: &State<ServerState>) -> Template {
    let mut context = state.default_context();
    if let Some(flash) = flash_msg_opt {
        context.insert("alert_type", &flash.kind());
        context.insert("alert_msg", &flash.message());
    }
    if let Some(user) = user_opt {
        context.insert("user", &user);
    }
    context.insert("err_msg", &message);
    layout("index", &context.into_json())
}

/// Following best practice, all static files (css, js, etc.)
/// are placed in the folder static, 
/// but still preventing directory traversal attacks
#[get("/static/<file..>", rank=10)]
async fn static_files(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new("static/").join(file)).await.ok()
}

/// As a first proxy, catch errors here
#[get("/err?<msg>")]
async fn error_msg(msg: String, user_opt: Option<UserCookie>, state: &State<ServerState>) -> Template {
    let mut context = state.default_context();
    context.insert("alert_type", "danger");
    context.insert("alert_msg", &msg);
    if let Some(user) = user_opt {
        context.insert("user", &user);
    } 
    layout("index", &context.into_json())
}

#[launch]
async fn rocket() -> _ {
    let matches = App::new("qualinvest")
        .setting(AppSettings::ColoredHelp)
        .version("0.3.0")
        .author("Mark Beinker <mwb@quantlink.de>")
        .about("Tools for quantitative analysis and management of financial asset portfolios")
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .value_name("file")
                .help("Sets a custom config file")
                .takes_value(true)
        )
        .arg(
            Arg::with_name("debug")
                .short("d")
                .long("debug")
                .help("Prints additional information for debugging purposes")
        ).get_matches();

    let config = matches.value_of("config").unwrap_or("qualinvest.toml");
    let config_file = fs::read_to_string(config).unwrap();
    let mut config: Config = toml::from_str(&config_file).unwrap();
    
    if matches.is_present("debug") {
        config.debug = true;
    }

    // Set up database
    let postgres_url = format!(
        "postgresql://{db_name}?user={user}&password={password}&sslmode=disable",
        db_name=config.db.name, user=config.db.user, password=config.db.password
    );
    println!("postgreSQL url: '{}'", postgres_url);
    let postgres_db = PostgresDB::new(&postgres_url).await.unwrap();

    // Set up all filters for tera
    filter::set_filter();

    let rocket_config = if config.debug {
        Figment::from(rocket::Config::default())
        .merge(("port",config.server.port.unwrap_or(8000)))
    } else {
        Figment::from(rocket::Config::default())
        .merge(("port",config.server.port.unwrap_or(8000)))
        .merge(("secret_key",config.server.secret_key.unwrap()))
    };
    let templates = filter::set_filter();
    let server_state = ServerState {
        postgres_db: Arc::new(postgres_db),
        doc_path: config.pdf.doc_path.clone(),
        market_data: config.market_data
    };

    // Normalize rel_path, i.e. either "" or "<some path>/" such that format!("/{}<rest of path>", rel_path) is valid
    let rel_path = match config.server.relative_path {
        Some(path) => {
            let mut rel_path = path;
            while rel_path.ends_with('/') { rel_path.pop(); }
            if !rel_path.starts_with('/') {
                rel_path = format!("/{}", rel_path);
            }
            rel_path
        },
        None => "".to_string(),
    };
    ServerState::set_base(rel_path);

    rocket::custom(rocket_config)
        .attach(templates)
        .manage(server_state)
        .mount(ServerState::base(), routes![
            logged_in,
            login,
            retry_login_user,
            retry_login_flash,
            process_login,
            logout,
            index,
            static_files,
            error_msg,
            position::position,
            transactions::transactions,
            transactions::edit_transaction,
            transactions::delete_transaction,
            transactions::pdf_upload,
            transactions::pdf_upload_form,
            transactions::update_transaction,
            asset::analyze_asset,
            asset::assets,
            asset::delete_asset,
            asset::edit_asset,
            asset::save_asset,
            quotes::show_quotes,
            quotes::update_asset_quote,
            quotes::delete_quote,
            quotes::renew_history,
            quotes::new_quote,
            quotes::add_new_quote,
            ticker::show_ticker,
            ticker::edit_ticker,
            ticker::save_ticker,
            ticker::delete_ticker,
            user_settings::show_settings,
            user_settings::save_settings,
            accounts::accounts,
            accounts::add_account,
            accounts::delete_account,
            accounts::add_user,
            accounts::delete_user,
            accounts::user_accounts,
        ])
}
