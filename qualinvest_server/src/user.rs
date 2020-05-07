
use rocket::{Request, Outcome};
use rocket::request::FromRequest;
use std::collections::HashMap;
use finql::data_handler::DataError;
use finql::postgres_handler::PostgresDB;
use super::auth::authorization::*;

/// User information as stored in database
#[derive(Debug)]
pub struct User {
    pub id: Option<usize>,
    pub name: String,
    pub display: Option<String>,
    pub salt_hash: String,
    pub is_admin: bool,
}


/// The UserCookie type is used to indicate a user has logged in as an user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserCookie {
    pub userid: usize,
    pub username: String,
    pub display: Option<String>,
    pub is_admin: bool,
}

/// The UserForm type is used to process a user attempting to login as an user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserForm {
    pub username: String,
    pub password: String,
    pub redirect: String,
}

impl CookieId for UserCookie {
    fn cookie_id<'a>() -> &'a str {
        "plain_acid"
    }
}

impl CookieId for UserForm {
    fn cookie_id<'a>() -> &'a str {
        "plain_acid"
    }
} 

impl AuthorizeCookie for UserCookie {
    /// The store_cookie() method should contain code that
    /// converts the specified data structure into a string
    /// 
    /// This is likely to be achieved using one of the serde
    /// serialization crates.  Personally I would use either
    /// serde_json or serde's messagepack implementation ( rmp-serde [rmps]).
    /// 
    /// Json is portable and human readable.  
    /// 
    /// MsgPack is a binary format, and while not human readable is more
    /// compact and efficient.
    fn store_cookie(&self) -> String {
        ::serde_json::to_string(self).expect("Could not serialize")
    }
    
    
    /// The retrieve_cookie() method deserializes a string
    /// into a cookie data type.
    /// 
    /// Again, serde is likely to be used here.
    /// Either the messagepack or json formats would work well here.
    /// 
    /// Json is portable and human readable.  
    /// 
    /// MsgPack is a binary format, and while not human readable is more
    /// compact and efficient.
    #[allow(unused_variables)]
    fn retrieve_cookie(string: String) -> Option<Self> {
        let mut des_buf = string.clone();
        let des: Result<UserCookie, _> = ::serde_json::from_str(&mut des_buf);
        if let Ok(cooky) = des {
            Some(cooky)
        } else {
            None
        }
    }
}

impl AuthorizeForm for UserForm {
    type CookieType = UserCookie;
    
    /// Authenticate the credentials inside the login form
    fn authenticate(&self, db: &mut dyn UserHandler) -> Result<Self::CookieType, AuthFail> {
        let user = db.get_user_by_credentials(&self.username, &self.password).
            ok_or(AuthFail::new(self.username.clone(), "Authentication failed.".to_string()))?;
        if user.id.is_none() { return Err(AuthFail::new(self.username.clone(), "Authentication failed.".to_string())); }
        Ok(UserCookie {
            userid: user.id.unwrap(),
            username: user.name,
            display: user.display,
            is_admin: user.is_admin,
        })
    }
    
    /// Create a new login form instance
    fn new_form(user: &str, pass: &str, extras: Option<HashMap<String, String>>) -> Self {
        let redirect = match extras {
            Some(map) => {
                match map.get("redirect") {
                    Some(redirect) => redirect.clone(),
                    None => "/login".to_string(),
                }
            },
            None => "/login".to_string(),
        };
        UserForm {
            username: user.to_string(),
            password: pass.to_string(),
            redirect,
        }
    }
    
}

impl<'a, 'r> FromRequest<'a, 'r> for UserCookie {
    type Error = ();
    
    /// The from_request inside the file defining the custom data types
    /// enables the type to be checked directly in a route as a request guard
    /// 
    /// This is not needed but highly recommended.  Otherwise you would need to use:
    /// 
    /// `#[get("/protected")] fn admin_page(admin: AuthCont<UserCookie>)`
    /// 
    /// instead of:
    /// 
    /// `#[get("/protected")] fn admin_page(admin: UserCookie)`
    fn from_request(request: &'a Request<'r>) -> ::rocket::request::Outcome<UserCookie,Self::Error>{
        let cid = UserCookie::cookie_id();
        let mut cookies = request.cookies();
        
        match cookies.get_private(cid) {
            Some(cookie) => {
                if let Some(cookie_deserialized) = UserCookie::retrieve_cookie(cookie.value().to_string()) {
                    Outcome::Success(
                        cookie_deserialized
                    )
                } else {
                    Outcome::Forward(())
                }
            },
            None => Outcome::Forward(())
        }
    }
}


fn get_salt_hash(db: &mut PostgresDB, user_id: usize) -> Option<String> {
    match db.conn.query_one(
        "SELECT salt_hash FROM users WHERE id = $1",
            &[&(user_id as i32)],
        ) {
            Err(_) => None,
            Ok(row) => {
                let salt_hash: String = row.get(0);
                Some(salt_hash)
            }

        }
}


pub trait UserHandler {
    /// Clean database by dropping all tables related to user management and run init_users
    fn clean_users(&mut self) -> Result<(), DataError>;

    /// Set up new tables for user management
    fn init_users(&mut self) -> Result<(), DataError>;

    /// Insert new account info in database, if it not yet exist
    fn insert_user(&mut self, user: &mut User, password: &str) -> Result<usize, DataError>;

    /// Get full user information if user name and password are valid
    fn get_user_by_credentials(&mut self, name: &str, password: &str) -> Option<User>;
    /// Get full user information for given user id
    fn get_user_by_id(&mut self, user_id: usize) -> Option<User>;
    /// Get user id for given name if it exists
    fn get_user_id(&mut self, name: &str) -> Option<usize>;
    /// Get user id for given name if user exists and is admin
    fn get_admin_id(&mut self, name: &str) -> Option<usize>;
    /// Get user id if user name and password are valid
    fn get_user_id_by_credentials(&mut self, name: &str, password: &str) -> Option<usize>; 

    /// Update user, but let password unchanged
    fn update_user(&mut self, user: &User) -> Result<(), DataError>;

    /// Update user password 
    fn update_password(&mut self, user: &mut User, password: &str) -> Result<(), DataError>;

    /// Remove all user information form data base
    fn delete_user(&mut self, user_id: usize) -> Result<(), DataError>;
}

impl UserHandler: AccountHandler for PostgresDB<'_> {
    /// Clean database by dropping all tables related to user management and run init_users
    fn clean_users(&mut self) -> Result<(), DataError> {
        self.conn.execute("DROP TABLE IF EXISTS users", &[])
           .map_err(|e| DataError::DataAccessFailure(e.to_string()))?;
        self.init_users()?;
        Ok(())
    }

    /// Set up new tables for user management
    fn init_users(&mut self) -> Result<(), DataError> {
        self.conn.execute(
        "CREATE TABLE IF NOT EXISTS users (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            display TEXT,
            salt_hash TEXT NOT NULL,
            is_admin BOOLEAN NOT NULL DEFAULT False,
            UNIQUE (name))",
        &[],
        )
        .map_err(|e| DataError::DataAccessFailure(e.to_string()))?;
        Ok(())
    }

    /// Insert new account info in database, if it not yet exist
    fn insert_user(&mut self, user: &mut User, password: &str) -> Result<usize, DataError> {
        let row = self.conn
            .query_one(
                "INSERT INTO users (name, display, salt_hash, is_admin) VALUES ($1, $2, crypt($3,get_salt('bf',8)), $4) RETURNING id",
                &[&user.name, &user.display, &password, &user.is_admin],
            )
            .map_err(|e| DataError::InsertFailed(e.to_string()))?;
        let id: i32 = row.get(0);
        user.salt_hash = get_salt_hash(self, id as usize).ok_or(DataError::InsertFailed("reading hash of just inserted user failed".to_string()))?;
        Ok(id as usize)
    }
    
    /// Get full user information if user name and password are valid
    fn get_user_by_credentials(&mut self, name: &str, password: &str) -> Option<User> {
        match self.conn.query_one(
                "SELECT id, name, display, salt_hash, is_admin FROM users WHERE name = $1 AND 
                salt_hash = crypt($2, salt_hash)",
                &[&name, &password],
            ) {
                Err(_) => None,
                Ok(row) => {
                    let id: i32 = row.get(0);
                    Some(User{
                        id: Some(id as usize),
                        name: row.get(1),
                        display: row.get(2),
                        salt_hash: row.get(3),
                        is_admin: row.get(4),
                    })
                }

            }
    }

     /// Get full user information for given user id
    fn get_user_by_id(&mut self, user_id: usize) -> Option<User> {
        match self.conn.query_one(
                "SELECT id, name, display, salt_hash, is_admin FROM users WHERE id = $1",
                &[&(user_id as i32)],
            ) {
                Err(_) => None,
                Ok(row) => {
                    let id: i32 = row.get(0);
                    Some(User{
                        id: Some(id as usize),
                        name: row.get(1),
                        display: row.get(2),
                        salt_hash: row.get(3),
                        is_admin: row.get(4),
                    })
                }

            }
    }

    /// Get user id for given name if it exists
    fn get_user_id(&mut self, name: &str) -> Option<usize> {
        match self.conn.query_one(
            "SELECT id FROM users WHERE name = $1",
            &[&name],
        ) {
            Err(_) => None,
            Ok(row) => {
                let id: i32 = row.get(0);
                Some(id as usize)
            },
        }
    }
    /// Get user id for given name if user exists and is admin
    fn get_admin_id(&mut self, name: &str) -> Option<usize> {
        match self.conn.query_one(
            "SELECT id FROM users WHERE name = $1 AND is_admin",
                &[&name],
            ) {
                Err(_) => None,
                Ok(row) => {
                    let id: i32 = row.get(0);
                    Some(id as usize)
                }

            }
    }
    /// Get user id if user name and password are valid
    fn get_user_id_by_credentials(&mut self, name: &str, password: &str) -> Option<usize> {
        match self.conn.query_one(
            "SELECT id FROM users WHERE name = $1 AND 
                salt_hash = crypt($2, salt_hash)",
                &[&name, &password],
            ) {
                Err(_) => None,
                Ok(row) => {
                    let id: i32 = row.get(0);
                    Some(id as usize)
                }

            }
    }

    /// Update user 
    fn update_user(&mut self, user: &User) -> Result<(), DataError> {
        if user.id.is_none() {
            return Err(DataError::NotFound(
                "not yet stored to database".to_string(),
            ));
        }
        let id = user.id.unwrap();
        self.conn
            .execute(
                "UPDATE users SET name=$2, display=$3, is_admin=$4 WHERE id=$1",
                &[
                    &(id as i32),
                    &user.name,
                    &user.display,
                    &user.is_admin,
                ],
            )
            .map_err(|e| DataError::InsertFailed(e.to_string()))?;
        Ok(())
    }

    /// Update user password
    fn update_password(&mut self, user: &mut User, new_password: &str) -> Result<(), DataError> {
        if user.id.is_none() {
            return Err(DataError::NotFound(
                "not yet stored to database".to_string(),
            ));
        }
        let id = user.id.unwrap();
        self.conn
            .execute(
                "UPDATE users SET salt_hash=crypt($3, get_salt('bf',8)) WHERE id=$1 AND 
                salt_hash = crypt($2, salt_hash)",
                &[
                    &(id as i32),
                    &user.salt_hash,
                    &new_password,
                ],
            )
            .map_err(|e| DataError::InsertFailed(e.to_string()))?;
        user.salt_hash = get_salt_hash(self, id).ok_or(DataError::InsertFailed("reading hash of just inserted user failed".to_string()))?;
        Ok(())
    }

    /// Remove all user information form data base
    fn delete_user(&mut self, user_id: usize) -> Result<(), DataError> {
        self.conn
            .execute("DELETE FROM users WHERE id=$1;", &[&(user_id as i32)])
            .map_err(|e| DataError::InsertFailed(e.to_string()))?;
        Ok(())
    }
}

