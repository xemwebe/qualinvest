use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(feature = "ssr")] {
        use finql::postgres::PostgresDB;
        use leptos::context::use_context;
        use leptos::prelude::ServerFnError;

        pub fn get_db() -> Result<PostgresDB, ServerFnError> {
            use_context::<PostgresDB>()
                .ok_or_else(|| ServerFnError::ServerError("Database is missing.".into()))
        }
    }
}
