/// View performance comparison of different assets / portfolios

use super::rocket_uri_macro_login;
use rocket::response::Redirect;
use rocket::State;
use rocket_dyn_templates::Template;
use chrono::{DateTime, Local, TimeZone};
use finql::datatypes::{Asset, AssetHandler, DataItem, QuoteHandler, Stock};

use super::ServerState;
use crate::layout::layout;
use crate::user::UserCookie;
use crate::form_types::AssetListItem;


#[derive(Debug,Serialize)]
struct Graph {
    name: String,
    values: Vec<f64>,
}

#[get("/performance?<asset_ids>&<message>")]
pub async fn performance(
    asset_ids: Option<Vec<i32>>,
    message: Option<String>,
    user_opt: Option<UserCookie>,
    state: &State<ServerState>,
) -> Result<Template, Redirect> {
    if user_opt.is_none() {
        return Err(Redirect::to(uri!(
            ServerState::base(),
            login(Some("performance"))
        )));
    }
    let user = user_opt.unwrap();

    let mut message = message;
    let mut context = state.default_context();


    let db = state.postgres_db.clone();
    if let Ok(mut asset_list) = db.get_asset_list().await{ 
        asset_list.sort_by(|a, b| a.name.cmp(&b.name));
        context.insert("assets", &asset_list);
    } else {
        message = Some("No assets found!".to_string());
    }

//    let user_accounts = user.get_accounts(db.clone()).await;

    // test demo graph
    let dates = vec![
        Local.ymd(2022, 1, 1).and_hms_milli(0, 0, 0, 0), 
        Local.ymd(2022, 1, 1).and_hms_milli(0, 0, 0, 0), 
        Local.ymd(2022, 1, 1).and_hms_milli(0, 0, 0, 0), 
    ];
    let graphs = vec![
        Graph{ name: "graph1".to_string(), values: vec![ 1., 4., 2.] },
        Graph{ name: "graph1".to_string(), values: vec![ 3., 2., 5.] },
        Graph{ name: "graph1".to_string(), values: vec![ 2., 4., 2.] },
    ];
    context.insert("dates", &dates);
    context.insert("graphs", &graphs);
    context.insert("asset_ids", &asset_ids);
    context.insert("user", &user);

    context.insert("err_msg", &message);
    Ok(layout("performance", &context.into_json()))
}
