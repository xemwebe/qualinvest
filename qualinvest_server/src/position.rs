use rocket::response::Redirect;
use rocket::State;
use rocket_dyn_templates::Template;

use super::rocket_uri_macro_login;
use super::ServerState;
use crate::layout::layout;
use crate::user::UserCookie;
use finql::{
    datatypes::{AssetHandler, CurrencyISOCode},
    portfolio::{PortfolioPosition, PositionTotals},
};
use qualinvest_core::{position::calculate_position_for_period_for_accounts, user::UserHandler};

#[get("/position")]
pub async fn position(
    user_opt: Option<UserCookie>,
    state: &State<ServerState>,
) -> Result<Template, Redirect> {
    if user_opt.is_none() {
        return Err(Redirect::to(uri!(
            ServerState::base(),
            login(Some("position"))
        )));
    }
    let user = user_opt.unwrap();

    let db = state.postgres_db.clone();
    let currency = db
        .get_or_new_currency(CurrencyISOCode::new("EUR").unwrap())
        .await
        .unwrap();

    let user_settings = db.get_user_settings(user.userid).await;
    let period_end = user_settings.period_end;
    let period_start = user_settings.period_start;

    let mut context = state.default_context();

    let pnl = calculate_position_for_period_for_accounts(
        currency,
        &user_settings.account_ids,
        period_start,
        period_end,
        db,
    )
    .await;

    if let Ok((position, totals)) = pnl
    {
        context.insert("positions", &position);
        context.insert("totals", &totals);
    } else {
        context.insert("positions", &PortfolioPosition::new(currency));
        context.insert("totals", &PositionTotals::default());
        context.insert("err_msg", "Calculation of position failed");
    }

    context.insert("user", &user);
    Ok(layout("position", &context.into_json()))
}
