///! # qualinvest_server
///! 
///! This library is part of a set of tools for quantitative investments.
///! For mor information, see [qualinvest on github](https://github.com/xemwebe/qualinvest)
///!

#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use] extern crate rocket;
#[macro_use] extern crate rocket_contrib;
#[macro_use] extern crate serde;

use clap::{App, AppSettings, Arg};
use std::ops::DerefMut;

use rocket::State;
use rocket::config::{Value,Environment};
use rocket::http::{Cookie, Cookies};
use rocket::response::{NamedFile, Redirect, Flash};
use rocket::request::{FlashMessage, Form};
use rocket_contrib::databases::postgres;
use rocket_contrib::templates::Template;

use finql::postgres_handler::PostgresDB;
use qualinvest_core::Config;
use std::fs;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tera;

mod position;
mod transactions;
mod helper;
mod filter;
mod auth;
mod user;
mod layout;
use auth::authorization::*;
use user::*;
use layout::*;

#[database("qlinvest_db")]
pub struct QlInvestDbConn(postgres::Client);


/// The `logged_in()` method queries the database for the username specified
/// in the cookie.  In this instance all of the data in the database is also
/// contained in the cookie, making a database operation unnecessary, however
/// this is just an example to show how to connect to a database.
#[get("/login", rank = 1)]
fn logged_in(user: UserCookie, state: State<Config>) -> Template {
    let mut context = default_context(&state);
    context.insert("user", &user);
    layout("index", &context.into_json())
}

#[get("/login?<redirect>", rank = 2)]
fn login(redirect: Option<String>, state: State<Config>) -> Template {
    let mut context = default_context(&state);
    context.insert("login_url", "/login");
    if let Some(redirect) = redirect {
        context.insert("redirect", &redirect);
    }
    layout("login", &context.into_json())
}


/// if there is a user query string, and an optional flash message
/// display an optional flash message indicating why the login failed
/// and the login screen with user filled in
#[get("/login?<user>&<redirect>")]
fn retry_login_user(user: UserQuery, redirect: Option<String>, flash_msg_opt: Option<FlashMessage>, state: State<Config>) -> Template {
    let alert;
    if let Some(flash) = flash_msg_opt {
        alert = flash.msg().to_string();
    } else { 
        alert = "".to_string();
    }
    let mut context = default_context(&state);
    context.insert("login_url", "/login");
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
fn retry_login_flash(redirect: Option<String>, flash_msg: FlashMessage, state: State<Config>) -> Template {
    let alert = flash_msg.msg();

    let mut context = default_context(&state);
    context.insert("login_url", "/login");
    context.insert("alert_type", "danger");
    context.insert("alert_msg", &alert);
    if let Some(redirect) = redirect {
        context.insert("redirect", &redirect);
    }
    layout("login", &context.into_json())
}

#[post("/login", data = "<form>")]
fn process_login(form: Form<LoginCont<UserForm>>, mut cookies: Cookies, mut qldb: QlInvestDbConn) -> Result<Redirect, Flash<Redirect>> {
    let mut db = PostgresDB{ conn: qldb.0.deref_mut() };
    let inner = form.into_inner();
    let login = inner.form;
    login.flash_redirect(login.redirect.clone(), "/login", &mut cookies, &mut db)
}

#[get("/logout")]
fn logout(user: Option<UserCookie>, mut cookies: Cookies) -> Result<Flash<Redirect>, Redirect> {
    if let Some(_) = user {
        cookies.remove_private(Cookie::named(UserCookie::cookie_id()));
        Ok(Flash::success(Redirect::to("/"), "Successfully logged out."))
    } else {
        Err(Redirect::to("/login"))
    }
}


#[get("/")]
fn index(user_opt: Option<UserCookie>, flash_msg_opt: Option<FlashMessage>, state: State<Config>) -> Template {
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
    let mut context = default_context(&state);
    context.insert("alert_type", &alert_type);
    context.insert("alert_message", &alert_msg);
    if let Some(user) = user_opt {
        context.insert("user", &user);
    } 
    layout("index", &context.into_json())
}

/// Following best practice, all static files (css, js, etc.)
/// are placed in the folder static, 
/// but still preventing directory traversal attacks
#[get("/static/<file..>", rank=10)]
fn static_files(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new("static/").join(file)).ok()
}

/// As a first proxy, catch errors here
#[get("/err/<msg>")]
fn error_msg(msg: String, user_opt: Option<UserCookie>, state: State<Config>) -> Template {
    let mut context = default_context(&state);
    context.insert("alert_type", "danger");
    context.insert("alert_msg", &msg);
    if let Some(user) = user_opt {
        context.insert("user", &user);
    } 
    layout("index", &context.into_json())
}

fn main() {
    let matches = App::new("qualinvest")
        .setting(AppSettings::ArgRequiredElseHelp)
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
    let config: Config = toml::from_str(&config_file).unwrap();

    // Set up database
    let postgres_url = format!(
        "postgresql:///{db_name}?host={host}&user={user}&password={password}&sslmode=disable",
        host=config.db.host, db_name=config.db.name, user=config.db.user, password=config.db.password
    );
    let mut database_config = HashMap::new();
    let mut databases = HashMap::new();
    database_config.insert("url", Value::from(postgres_url.as_str()));
    databases.insert("qlinvest_db", Value::from(database_config));

    // Set up all filters for tera
    filter::set_filter();

    let rocket_config = rocket::Config::build(Environment::Development)
        .extra("databases", databases)
        .finalize()
        .unwrap();
    let templates = filter::set_filter();

    rocket::custom(rocket_config)
        .attach(QlInvestDbConn::fairing())
        .attach(templates)
        .manage(config)
        .mount("/", routes![
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
            filter::process_filter,
            static_files,
            error_msg
        ])
        .launch();
}


fn default_context(state: &State<Config>) -> tera::Context {
    let mut context = tera::Context::new();
    if let Some(rel_path) = state.server.relative_path.clone() {
        context.insert("relpath", &rel_path);
    } else {
        context.insert("relpath", "");
    }
    context
}