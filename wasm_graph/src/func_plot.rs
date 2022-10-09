use crate::DrawResult;
use chrono::{DateTime, Datelike, Local, NaiveDate, NaiveDateTime};
use plotters::prelude::*;
#[cfg(not(target_family = "wasm"))]
use plotters_bitmap::BitMapBackend;
#[cfg(target_family = "wasm")]
use plotters_canvas::CanvasBackend;
use serde_json;
use std::time::{Duration, UNIX_EPOCH};
use crate::log;

fn min_max_val<T: PartialOrd + GenericConst<T> + Copy>(values: &[T]) -> (T, T) {
    if values.is_empty() {
        return (T::zero(), T::one());
    }
    let mut min_val = values[0];
    let mut max_val = values[0];
    for val in values {
        if min_val > *val {
            min_val = *val;
        }
        if max_val < *val {
            max_val = *val;
        }
    }
    (min_val, max_val)
}

trait GenericConst<T> {
    fn one() -> T;
    fn zero() -> T;
}

impl GenericConst<i64> for i64 {
    fn one() -> i64 {
        1
    }
    fn zero() -> i64 {
        0
    }
}

impl GenericConst<f32> for f32 {
    fn one() -> f32 {
        1.
    }
    fn zero() -> f32 {
        0.
    }
}

fn calc_time_range(times: &[i64]) -> (DateTime<Local>, DateTime<Local>) {
    let (min_time, max_time) = min_max_val(times);
    let min_date =
        NaiveDateTime::from_timestamp(min_time / 1000, (min_time % 1000) as u32 * 1000).date();
    let min_date = NaiveDate::from_ymd(min_date.year(), min_date.month(), 1).and_hms(0, 0, 0);
    let max_date =
        NaiveDateTime::from_timestamp(max_time / 1000, (max_time % 1000) as u32 * 1000).date();
    let (mut year, mut month) = (max_date.year(), max_date.month());
    if month == 12 {
        year += 1;
        month = 1;
    } else {
        month += 1;
    }
    let max_date = NaiveDate::from_ymd(year, month, 1).and_hms(0, 0, 0);
    let min_date = DateTime::<Local>::from(
        UNIX_EPOCH + Duration::from_millis(min_date.timestamp_millis() as u64),
    );
    let max_date = DateTime::<Local>::from(
        UNIX_EPOCH + Duration::from_millis(max_date.timestamp_millis() as u64),
    );

    (min_date, max_date)
}

fn fmt_date_time(date: &DateTime<Local>) -> String {
    format!("{}.{}.{}", date.day(), date.month(), date.year())
}

/// Draw graph given (x,y) series
pub fn draw(
    canvas_id: &str,
    title: &str,
    times: &[i64],
    values: &[f32],
    names_json: &str,
) -> DrawResult<impl Fn((i32, i32)) -> Option<(i64, f32)>> {
    let names: Vec<String> = serde_json::from_str(names_json)?;
    log!("start draw: title: {}, times: {:?}, values: {:?}, names: {}", title, times, values, names_json);
    #[cfg(not(target_family = "wasm"))]
    let backend = BitMapBackend::new(canvas_id, (1280, 1024));
    #[cfg(target_family = "wasm")]
    let backend = CanvasBackend::new(canvas_id).expect("cannot find canvas");
    let root = backend.into_drawing_area();
    let font: FontDesc = ("sans-serif", 14.0).into();

    root.fill(&WHITE)?;

    let len_series = values.len() / names.len();
    let (min_time, max_time) = calc_time_range(times);
    let time_range = min_time..max_time;
    let (min_val, max_val) = min_max_val(values);
    let y_range = min_val..max_val;

    let mut chart = ChartBuilder::on(&root)
        .margin::<u32>(50)
        .caption(title, font)
        .x_label_area_size::<u32>(80)
        .y_label_area_size::<u32>(80);
    
    let time_span = max_time - min_time;
    if time_span < chrono::days(50) {
        chart.build_cartesian_2d(time_range.daily(), y_range)?;
    } else if time_span < Duration::months(50) {
        chart.build_cartesian_2d(time_range.monthly(), y_range)?;
    } else {
        chart.build_cartesian_2d(time_range.yearly(), y_range)?;
    }
    chart
        .configure_mesh()
        .x_labels(10)
        .x_desc("date")
        .x_label_formatter(&fmt_date_time)
        .disable_x_mesh()
        .y_desc("price")
        .y_labels(10)
        .label_style(("sans-serif", 16))
        .axis_desc_style(("sans-serif", 20))
        .draw()?;

    static COLORS: [&RGBColor; 5] = [&BLUE, &GREEN, &RED, &CYAN, &MAGENTA];
    let mut color_index: usize = 0;
    let mut idx = 0;
    for name in names {
        chart
            .draw_series(LineSeries::new(
                times
                    .iter()
                    .zip(values[idx..idx + len_series].iter())
                    .map(|(x, y)| {
                        (
                            DateTime::<Local>::from(UNIX_EPOCH + Duration::from_millis(*x as u64)),
                            *y,
                        )
                    }),
                &COLORS[color_index],
            ))?
            .label(name)
            .legend(move |(x, y)| {
                Rectangle::new([(x, y - 5), (x + 10, y + 5)], &COLORS[color_index])
            });
        idx += len_series;
        color_index = (color_index + 1) % COLORS.len();
    }

    chart
        .configure_series_labels()
        .background_style(&WHITE.mix(0.8))
        .border_style(&BLACK)
        .draw()?;

    root.present()?;

    #[cfg(not(target_family = "wasm"))]
    return Ok(move |(_,_)| None);
    #[cfg(target_family = "wasm")]   
    {
        let coord_convert = chart.into_coord_trans();
        return Ok(move |(x, y)| coord_convert((x, y)).map(|(t, v)| (t.timestamp_millis(), v)));
    }
}
