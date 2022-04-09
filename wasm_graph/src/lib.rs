use wasm_bindgen::prelude::*;

pub mod func_plot;
pub mod utils;

pub fn set_panic_hook() {
    // When the `console_error_panic_hook` feature is enabled, we can call the
    // `set_panic_hook` function at least once during initialization, and then
    // we will get better error messages if our code ever panics.
    //
    // For more details see
    // https://github.com/rustwasm/console_error_panic_hook#readme
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

/// Type alias for the result of a drawing function.
pub type DrawResult<T> = Result<T, Box<dyn std::error::Error>>;

/// Type used on the JS side to convert screen coordinates to chart
/// coordinates.
#[wasm_bindgen]
pub struct Chart {
    convert: Box<dyn Fn((i32, i32)) -> Option<(i64, f64)>>,
}

/// Result of screen to chart coordinates conversion.
#[wasm_bindgen]
pub struct Point {
    pub x: i64,
    pub y: f64,
}

#[wasm_bindgen]
impl Chart {
    /// Draw performance graph
    pub fn performance_graph(canvas_id: &str, title: &str, x_axis: &[i64], y_axis: &[f32]) -> Result<Chart, JsValue> {
        set_panic_hook();
        let map_coord = func_plot::draw(canvas_id, title, x_axis, y_axis).map_err(|err| err.to_string())?;
        Ok(Chart {
            convert: Box::new(move |coord| map_coord(coord).map(|(x, y)| (x.into(), y.into()))),
        })
    }
    /// This function can be used to convert screen coordinates to
    /// chart coordinates.
    pub fn coord(&self, x: i32, y: i32) -> Option<Point> {
        (self.convert)((x, y)).map(|(x, y)| Point { x, y })
    }
}
