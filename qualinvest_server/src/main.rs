#![feature(proc_macro_hygiene, decl_macro)]

use clap::{App, AppSettings, Arg};
use std::str::FromStr;
use std::ops::DerefMut;
use chrono::{DateTime,Local};
use rocket_contrib::json::Json;
use rocket_contrib::databases::postgres;
use rocket::config::{Value,Environment};
use finql::Currency;
use finql::data_handler::TransactionHandler;
use finql::postgres_handler::PostgresDB;
use qualinvest_core::position::{calc_position,PortfolioPosition};
use qualinvest_core::accounts::AccountHandler;
use qualinvest_core::Config;
use std::fs;
use std::collections::HashMap;

#[macro_use] extern crate rocket;
#[macro_use] extern crate rocket_contrib;

#[database("qlinvest_db")]
struct QlInvestDbConn(postgres::Client);

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[get("/position?<account>")]
fn position(account: Option<usize>, mut qldb: QlInvestDbConn) -> Json<PortfolioPosition> {
    let currency = Currency::from_str("EUR").unwrap();
    let mut db = PostgresDB{ conn: qldb.0.deref_mut() };
    let transactions = match account {
        Some(account_id) => db
            .get_all_transactions_with_account(account_id)
            .unwrap(),
        None => db.get_all_transactions().unwrap(),
    };
    let mut position = calc_position(currency, &transactions).unwrap();
    position.get_asset_names(&mut db).unwrap();
    
    let time = DateTime::from(Local::now());
    position.add_quote(time, &mut db).unwrap();
    
    Json(position)
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
                .required(false)
        )
        .arg(
            Arg::with_name("debug")
                .short("d")
                .long("debug")
                .help("Prints additional information for debugging purposes")
                .required(false)
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

    let rocket_config = rocket::Config::build(Environment::Development)
        .extra("databases", databases)
        .finalize()
        .unwrap();

    rocket::custom(rocket_config)
        .attach(QlInvestDbConn::fairing())
        .mount("/", routes![index])
        .mount("/", routes![position])
        .launch();
}
