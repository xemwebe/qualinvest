use crate::DrawResult;
use plotters::prelude::*;
use plotters_canvas::CanvasBackend;
use plotters_bitmap::BitMapBackend;
use chrono::{DateTime, Local, NaiveDateTime, NaiveDate, Datelike};
use std::time::{UNIX_EPOCH, Duration};

fn min_max_val<T: PartialOrd+GenericConst<T>+Copy>(values: &[T]) -> (T, T) {
    if values.is_empty() {
    return (T::zero(), T::one());
    }
    let mut min_val = values[0];
    let mut max_val = values[0];
    for val in values {
        if min_val > *val { min_val = *val }
        if max_val < *val { max_val = *val}
    }
    (min_val, max_val)
}

trait GenericConst<T> {
    fn one() -> T;
    fn zero() -> T;
}

impl GenericConst<i64> for i64 {
    fn one() -> i64 { 1 }
    fn zero() -> i64 { 0 }
}

impl GenericConst<f32> for f32 {
    fn one() -> f32 { 1. }
    fn zero() -> f32 { 0. }
}

fn calc_time_range(times: &[i64]) -> 
    (DateTime<Local>, DateTime<Local>) {
    let (min_time, max_time) = min_max_val(times);
    let min_date = NaiveDateTime::from_timestamp(min_time/1000, (min_time % 1000) as u32 * 1000 ).date();
    let min_date = NaiveDate::from_ymd(min_date.year(), min_date.month(),1).and_hms(0,0,0);
    let max_date = NaiveDateTime::from_timestamp(max_time/1000, (max_time % 1000) as u32 * 1000).date();
    let (mut year, mut month) = (max_date.year(), max_date.month());
    if month == 12 {
        year += 1;
        month = 1;
    } else {
        month += 1;
    }
    let max_date = NaiveDate::from_ymd(year, month, 1).and_hms(0,0,0);
    let min_date = DateTime::<Local>::from(UNIX_EPOCH + Duration::from_millis(min_date.timestamp_millis() as u64));
    let max_date = DateTime::<Local>::from(UNIX_EPOCH + Duration::from_millis(max_date.timestamp_millis() as u64));

    (min_date, max_date)
}

fn calc_time_grid(min_time: i64, max_time: i64) -> Vec<(i64, String)> {
    let mut time_grid = vec![];

    let mut year = NaiveDateTime::from_timestamp(min_time/1000, (min_time % 1000) as u32 * 1000 ).year();
    let last_year = 1 + NaiveDateTime::from_timestamp(max_time/1000, (max_time % 1000) as u32 * 1000 ).year();
    while year <= last_year{
        time_grid.push( (NaiveDate::from_ymd(year, 1, 1).and_hms(0,0,0).timestamp_millis(),
            format!("{}", year)));
        year += 1;
    }
    time_grid
}

fn fmt_date_time(date: &DateTime<Local>) -> String {
    //let date = NaiveDateTime::from_timestamp(*t/1000, (*t % 1000) as u32 * 1000).date();
    //format!("{}.{}.{}", date.day(), date.month(), date.year())
    format!("{}.{}.{}", date.day(), date.month(), date.year())
}

/// Draw graph given (x,y) series
pub fn draw(canvas_id: &str, title: &str, times: &[i64], values: &[f32]) -> DrawResult<impl Fn((i32, i32)) -> Option<(i64, f32)>> {
    let backend = CanvasBackend::new(canvas_id).expect("cannot find canvas");
    let root = backend.into_drawing_area();
    let font: FontDesc = ("sans-serif", 14.0).into();

    root.fill(&WHITE)?;

    let (min_time, max_time) = calc_time_range(times);
    let time_range = min_time..max_time;
    let (min_val, max_val) = min_max_val(values);
    let y_range = min_val..max_val;

    let mut chart = ChartBuilder::on(&root)
        .margin::<u32>(50)
        .caption(title, font)
        .x_label_area_size::<u32>(80)
        .y_label_area_size::<u32>(80)
        .build_cartesian_2d(time_range.yearly(), y_range)?;

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

    chart.draw_series(LineSeries::new(
        times.into_iter().zip(values.into_iter()).map(|(x,y)| (
            DateTime::<Local>::from(UNIX_EPOCH + Duration::from_millis(*x as u64)),*y)),
        &RED,
    ))?;

    root.present()?;

    let coord_convert = chart.into_coord_trans();
    return Ok(move |(x,y)| {
        coord_convert((x,y)).map(|(t,v)| (t.timestamp_millis(), v) )
    });
}

/// Draw graph given (x,y) series
#[cfg(not(target_family="wasm"))]
pub fn draw_bmp(file_name: &str, title: &str, times: &[i64], values: &[f32]) -> Result<(), Box<dyn std::error::Error>> {
    println!("x-Range: {}-{}", times[0], times.last().unwrap());
    let backend = BitMapBackend::new(file_name, (1280,1024));
    let root = backend.into_drawing_area();
    let font: FontDesc = ("sans-serif", 14.0).into();

    root.fill(&WHITE)?;

    let (min_time, max_time) = calc_time_range(times);
    let time_range = min_time..max_time;
    let (min_val, max_val) = min_max_val(values);
    let y_range = min_val..max_val;

    let mut chart = ChartBuilder::on(&root)
        .margin::<u32>(50)
        .caption(title, font)
        .x_label_area_size::<u32>(80)
        .y_label_area_size::<u32>(80)
        .build_cartesian_2d(time_range, y_range)?;

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

        chart.draw_series(LineSeries::new(
            times.into_iter().zip(values.into_iter()).map(|(x,y)| (
                DateTime::<Local>::from(UNIX_EPOCH + Duration::from_millis(*x as u64)),*y)),
            &RED,
        ))?;

    // let time_grid = calc_time_grid(min_time.timestamp_millis(), max_time.timestamp_millis());
    // for (ix, label) in time_grid {
    //     let label_element = |x: i64, y: f32| {
    //         let (xi, yi) = chart.backend_coord(&(x, y));
    //         return EmptyElement::at((xi, yi))
    //             + Text::new(
    //                 label,
    //                 (10, 0),
    //                 ("sans-serif", 15.0).into_font(),
    //             );
    //     };
    
    //     root.draw(&label_element(ix, min_val))?;
    // }
    root.present()?;
    // let area = chart.plotting_area();
    // println!("Drawing x range: {:?}", area.get_x_range());
    // println!("Drawing x pixel range: {:?}", area.get_x_axis_pixel_range());

    // let coord_convert = chart.into_coord_trans();
    // for x in 130..136 {
    //     println!("Result {:?}", coord_convert((x,300)));
    // }
    Ok(())
}
