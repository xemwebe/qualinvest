// A macro to provide `println!(..)`-style syntax for `console.log` logging.
#[cfg(target_family = "wasm")]
#[macro_export]
macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}

// A macro to log to standard out
#[cfg(not(target_family = "wasm"))]
#[macro_export]
macro_rules! log {
    ( $( $t:tt )* ) => {
        println!( $( $t )* );
    }
}
