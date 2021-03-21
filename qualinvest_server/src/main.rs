///! # qualinvest_server
///! 
///! This library is part of a set of tools for quantitative investments.
///! For mor information, see [qualinvest on github](https://github.com/xemwebe/qualinvest)
///!

#[macro_use] extern crate rocket;
#[macro_use] extern crate serde;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use clap::{App, AppSettings, Arg};

use rocket::State;
use rocket::http::{Cookie, CookieJar};
use rocket::response::{NamedFile, Redirect, Flash};
use rocket::request::{FlashMessage};
use rocket::form::Form;
use rocket_contrib::templates::Template;
use rocket::figment::Figment;
use tera::Value;
use tera;

use finql_postgres::PostgresDB;
use qualinvest_core::Config;

mod asset;
mod position;
mod transactions;
mod helper;
mod filter;
mod auth;
mod user;
mod layout;
mod form_types;
use auth::authorization::*;
use user::*;
use layout::*;

#[derive(Debug)]
pub struct ServerState {
    rel_path: String,
    postgres_db: Arc<PostgresDB>,
}

impl ServerState {
    pub fn default_context(&self) -> tera::Context {
        let mut context = tera::Context::new();
        context.insert("relpath", &self.rel_path);
        context
    }
}

/// The `logged_in()` method queries the database for the username specified
/// in the cookie.  In this instance all of the data in the database is also
/// contained in the cookie, making a database operation unnecessary, however
/// this is just an example to show how to connect to a database.
#[get("/login", rank = 1)]
fn logged_in(user: UserCookie, state: State<ServerState>) -> Template {
    let mut context = state.default_context();
    context.insert("user", &user);
    layout("index", &context.into_json())
}

#[get("/login?<redirect>", rank = 2)]
fn login(redirect: Option<String>, state: State<ServerState>) -> Template {
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
fn retry_login_user(user: UserQuery, redirect: Option<String>, flash_msg_opt: Option<FlashMessage>, state: State<ServerState>) -> Template {
    let alert;
    if let Some(flash) = flash_msg_opt {
        alert = flash.msg().to_string();
    } else { 
        alert = "".to_string();
    }
    let mut context = state.default_context();
    context.insert("user", &user.user);
    context.insert("alert_type", "danger");
    context.insert("alert_msg", &alert);
    if let Some(redirect) = redirect {
        context.insert("redirect", &redirect);
    }
    layout("login", &context.into_json())
}

/// if there is a flash message but no user query string
/// display why the login failed and display the login screen
#[get("/login?<redirect>", rank = 3)]
fn retry_login_flash(redirect: Option<String>, flash_msg: FlashMessage, state: State<ServerState>) -> Template {
    let alert = flash_msg.msg();

    let mut context = state.default_context();
    context.insert("alert_type", "danger");
    context.insert("alert_msg", &alert);
    if let Some(redirect) = redirect {
        context.insert("redirect", &redirect);
    }
    layout("login", &context.into_json())
}

#[post("/login", data = "<form>")]
fn process_login(form: Form<LoginCont<UserForm>>, mut cookies: CookieJar, 
        state: State<'_,ServerState>) -> Result<Redirect, Flash<Redirect>> {
    let db = state.postgres_db;
    let inner = form.into_inner();
    let login = inner.form;
    login.flash_redirect(login.redirect.clone(), format!("{}/login", state.rel_path), &mut cookies, &db)
}

#[get("/logout")]
fn logout(user: Option<UserCookie>, mut cookies: CookieJar, state: State<ServerState>) -> Result<Flash<Redirect>, Redirect> {
    if let Some(_) = user {
        cookies.remove_private(Cookie::named(UserCookie::cookie_id()));
        Ok(Flash::success(Redirect::to(format!("{}/",state.rel_path)), "Successfully logged out."))
    } else {
        Err(Redirect::to(format!("{}/login", state.rel_path)))
    }
}


#[get("/")]
fn index(user_opt: Option<UserCookie>, flash_msg_opt: Option<FlashMessage>, state: State<ServerState>) -> Template {
    let (alert_type, alert_msg) = if let Some(flash) = flash_msg_opt {
        match flash.name() {
            "success" => ("success", flash.msg().to_string()),
            "warning" => ("warning", flash.msg().to_string()),
            "error" => ("error", flash.msg().to_string()),
            _ => ("info", flash.msg().to_string()),
        }
    } else {
        ("info", "".to_string())
    };
    let mut context = state.default_context();
    context.insert("alert_type", &alert_type);
    context.insert("alert_msg", &alert_msg);
    if let Some(user) = user_opt {
        context.insert("user", &user);
    } 
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
fn error_msg(msg: String, user_opt: Option<UserCookie>, state: State<ServerState>) -> Template {
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
        "postgresql:///{db_name}?user={user}&password={password}&sslmode=disable",
        db_name=config.db.name, user=config.db.user, password=config.db.password
    );
    let postgres_db = PostgresDB::new(&postgres_url).await.unwrap();

    // Set up all filters for tera
    filter::set_filter();

    let rocket_config = if config.debug {
        Figment::from(rocket::Config::default())
        .merge(("port",config.server.port.unwrap_or(8000)))
    } else {
        if config.server.secret_key.is_none() {
            println!("Please set a secret key for production environment!");
            return ();
        }
        Figment::from(rocket::Config::default())
        .merge(("port",config.server.port.unwrap_or(8000)))
        .merge(("secret_key",config.server.secret_key.unwrap()))
    };
    let templates = filter::set_filter();

    let mount_path = match config.server.relative_path {
        Some(ref path) => path.clone(),
        None => "/".to_string(),
    };
    let server_state = ServerState {
        rel_path: mount_path.clone(),
        postgres_db: Arc::new(postgres_db)
    };
    rocket::custom(rocket_config)
        .attach(templates)
        .manage(server_state)
        .mount(&mount_path, routes![
            logged_in,
            login,
            retry_login_user,
            retry_login_flash,
            process_login,
            logout,
            index,
            position::position,
            transactions::transactions,
            transactions::new_transaction,
            transactions::edit_transaction,
            transactions::delete_transaction,
            transactions::process_transaction,
            asset::analyze_asset,
            filter::process_filter,
            static_files,
            error_msg
        ])
        .launch();
}
