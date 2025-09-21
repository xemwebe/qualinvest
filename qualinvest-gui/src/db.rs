use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(feature = "ssr")] {
        use finql::postgres::PostgresDB;
        use leptos::context::use_context;
        use leptos::prelude::ServerFnError;
        use log::{debug, error};

        pub fn get_db() -> Result<PostgresDB, ServerFnError> {
            debug!("Request database access from context.");
            use_context::<PostgresDB>()
                .ok_or_else(|| {
                    error!("Database is missing in context.");
                    ServerFnError::ServerError("Database is missing.".into())
                })
        }
    }
}
