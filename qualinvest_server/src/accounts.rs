/// Viewing and analyzing assets

use std::collections::HashMap;
use std::collections::HashSet;
use super::{rocket_uri_macro_login};
use rocket::form::{Form, FromForm};
use rocket::response::Redirect;
use rocket::State;
use rocket_dyn_templates::Template;

use super::ServerState;
use crate::layout::layout;
use crate::user::UserCookie;
use qualinvest_core::accounts::{Account, AccountHandler};
use qualinvest_core::user::{User, UserHandler};

/// Structure for storing information in accounts form
#[derive(Debug, Serialize, Deserialize, FromForm)]
pub struct AccountForm {
    pub id: Option<usize>,
    pub broker: String,
    pub account_name: String,
}

impl AccountForm {
    pub fn to_account(&self) -> Account {
        Account {
            id: self.id,
            account_name: self.account_name.clone(),
            broker: self.broker.clone(),
        }
    }
}

/// Structure for storing information users accounts
#[derive(Debug, Serialize, Deserialize, FromForm)]
pub struct UserAccountsForm {
    pub user_id: usize,
    pub accounts: Option<Vec<usize>>,
}

/// Structure for storing information in accounts form
#[derive(Debug, Serialize, Deserialize, FromForm)]
pub struct UserForm {
    pub id: Option<usize>,
    pub name: String,
    pub display: Option<String>,
    pub is_admin: bool,
    pub password: Option<String>,
}

impl UserForm {
    pub fn to_user(&self) -> User {
        User {
            id: self.id,
            name: self.name.clone(),
            display: self.display.clone(),
            is_admin: self.is_admin,
        }
    }
}

#[get("/accounts?<message>")]
pub async fn accounts(
    message: Option<String>,
    user_opt: Option<UserCookie>,
    state: &State<ServerState>,
) -> Result<Template, Redirect> {
    let user = user_opt.ok_or_else(|| Redirect::to(
        uri!(ServerState::base(), login(Some("accounts")))
    ))?;

    let db = state.postgres_db.clone();
    let users; 
    let mut accounts = Vec::new();
    let mut user_accounts: HashMap<usize, Vec<Account>> = HashMap::new();
    if user.is_admin {
        users = db.get_all_users().await;
        accounts = db.get_all_accounts().await;
        for u in &users {
            if let Some(uid) = u.id  {
                if let Ok(u_accounts) = db.get_user_accounts(uid).await {
                    user_accounts.insert(uid, u_accounts);
                }
            }
        }
        // For some reason, the code below makes the whole function fail to compile with apparently unrelated error message
        // for uid in users.iter().map(|u| u.id).flatten() {
        //     let ua = db.get_user_accounts(uid).await;
        //     if let Ok(u_accounts) = db.get_user_accounts(uid).await {
        //         user_accounts.insert(uid, u_accounts);
        //     }
        // }
    } else {
        if let Ok(u_accounts) = db.get_user_accounts(user.userid).await {
            user_accounts.insert(user.userid, u_accounts);
        }
        accounts.extend(user_accounts[&user.userid].clone());
        users = vec![User {
            id: Some(user.userid),
            name: user.username.clone(),
            display: user.display.clone(),
            is_admin: false,
        }];
    }

    let mut context = state.default_context().clone();
    context.insert("err_msg", &message);
    context.insert("user", &user);
    context.insert("users", &users);
    context.insert("accounts", &accounts);
    context.insert("user_accounts", &user_accounts);
    Ok(layout("accounts_and_users", &context.into_json()))
}

#[post("/account/add", data = "<form>")]
pub async fn add_account(
    form: Form<AccountForm>,
    user: UserCookie,
    state: &State<ServerState>,
) -> Result<Redirect, Redirect> {
    let db = state.postgres_db.clone();
    let account = &form.into_inner().to_account();
    if let Some(account_id) = account.id {
        if let Ok(u_accounts) = db.get_user_accounts(user.userid).await {
            if u_accounts.iter().filter(|a| a.id == Some(account_id)).next().is_none() {
                return Err(Redirect::to(uri!(ServerState::base(), accounts(Some("You can only update your own accounts")))));
            }
        }
        db.update_account(account).await
            .map_err(|e| {
                Redirect::to(uri!(ServerState::base(), accounts(Some(format!("Updating account failed: {}", e)))))})?;
    } else {
        let account_id = db.insert_account_if_new(account).await
            .map_err(|e| { Redirect::to(uri!(ServerState::base(), accounts(Some(format!("Adding account failed: {}", e)))))
            })?;
        if !user.is_admin {
            db.add_account_right(user.userid, account_id).await
                .map_err(|e| { Redirect::to(uri!(ServerState::base(), accounts(Some(format!("Adding user rights to access new account failed: {}", e)))))
            })?;
        }
    }

    Ok(Redirect::to(uri!(ServerState::base(), accounts(Option::<String>::None))))
}

#[get("/account/delete?<id>")]
pub async fn delete_account(
    id: usize,
    user: UserCookie,
    state: &State<ServerState>,
) -> Result<Redirect, Redirect> {
    if !user.is_admin {
        return Err(Redirect::to(uri!(ServerState::base(), accounts(Some("You must be admin user to delete accounts!")))));
    }

    state.postgres_db.delete_account(id).await.map_err(|e| {
        Redirect::to(uri!(ServerState::base(), accounts(Some(format!("Delete account failed: {}", e)))))
    })?;

    Ok(Redirect::to(uri!(ServerState::base(), accounts(Option::<String>::None))))
}

#[post("/user/add", data = "<form>")]
pub async fn add_user(
    form: Form<UserForm>,
    user: UserCookie,
    state: &State<ServerState>,
) -> Result<Redirect, Redirect> {
    let user_form = &form.into_inner();
    let new_user = user_form.to_user();
    if let Some(user_id) = new_user.id {
        state.postgres_db.update_user(&new_user).await.map_err(|e| {
            Redirect::to(uri!(ServerState::base(), accounts(Some(format!("Updating user failed: {}", e)))))
        })?;
        if let Some(password) = &user_form.password {
            state
                .postgres_db
                .update_password(user_id, &password)
                .await
                .map_err(|e| {
                    Redirect::to(uri!(ServerState::base(), accounts(Some(format!("Updating user password failed: {}", e)))))
                })?;
        }
    } else {
        if !user.is_admin {
            return Err(Redirect::to(uri!(ServerState::base(), accounts(Some("You need to be admin to add a new user")))));
        }
        if let Some(password) = &user_form.password {
            state
                .postgres_db
                .insert_user(&new_user, &password)
                .await
                .map_err(|e| {
                    Redirect::to(uri!(ServerState::base(), accounts(Some(format!("Adding user failed: {}", e)))))
                })?;
        } else {
            return Err(Redirect::to(uri!(ServerState::base(), accounts(Some("You must set a password for new users!")))));
        }
    }

    Ok(Redirect::to(uri!(ServerState::base(), accounts(Option::<String>::None))))
}

#[get("/user/delete?<id>")]
pub async fn delete_user(
    id: usize,
    user: UserCookie,
    state: &State<ServerState>,
) -> Result<Redirect, Redirect> {
    if !user.is_admin {
        return Err(Redirect::to(uri!(ServerState::base(), accounts(Some("You must be admin user to delete users!")))));
    }

    state.postgres_db.delete_user(id).await.map_err(|e| {
        Redirect::to(uri!(ServerState::base(), accounts(Some(format!("Delete user failed: {}", e)))))
    })?;

    Ok(Redirect::to(uri!(ServerState::base(), accounts(Option::<String>::None))))
}

#[post("/user_accounts", data="<form>")]
pub async fn user_accounts(
    form: Form<UserAccountsForm>,
    user: UserCookie,
    state: &State<ServerState>,
) -> Result<Redirect, Redirect> {
    if !user.is_admin {
        return Err(Redirect::to(uri!(ServerState::base(), accounts(
            Some("You must be admin user to change other users accounts; to change your own accounts, use the accounts menu!")))));
    }

    let user_accounts = &form.into_inner();
    let old_accounts: HashSet<usize> = state.postgres_db.get_user_accounts(user_accounts.user_id).await
        .map_err(|e| Redirect::to(uri!(ServerState::base(), accounts(Some(format!("Updating users accounts user failed: {}", e))))))?
        .into_iter().map(|a| a.id).flatten().collect();
    let new_accounts: HashSet<usize> = user_accounts.accounts.iter().flatten().map(|id| *id).collect();
    for u in (&old_accounts - &new_accounts).into_iter() {
        state.postgres_db.remove_account_right(user_accounts.user_id, u).await
        .map_err(|e| Redirect::to(uri!(ServerState::base(), accounts(Some(format!("Updating users accounts user failed: {}", e))))))?;
    }
    for u in (&new_accounts - &old_accounts).into_iter() {
        state.postgres_db.add_account_right(user_accounts.user_id, u).await
        .map_err(|e| Redirect::to(uri!(ServerState::base(), accounts(Some(format!("Updating users accounts user failed: {}", e))))))?;
    }
    
    Ok(Redirect::to(uri!(ServerState::base(), accounts(Option::<String>::None))))
}
