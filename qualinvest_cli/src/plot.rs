use thiserror::Error;
use plotters::prelude::*;
use chrono::Datelike;

use finql::time_series::TimeSeries;
use finql::datatypes::date_time_helper::make_time;
use finql::calendar::last_day_of_month;

/// Error related to plotting graphs
#[derive(Error, Debug)]
pub enum PlotError {
    #[error("time series is empty")]
    EmptyTimeSeries,
    #[error("IO error")]
    IOError(#[from] std::io::Error),
    #[error("drawing error")]
    DrawingError(#[from] DrawingAreaErrorKind<std::io::Error>),
    #[error("time series error")]
    TimeSerieos(#[from] finql::time_series::TimeSeriesError),
}

pub fn make_plot(
    file_name: &str,
    title: &str,
    all_time_series: &[TimeSeries],
) -> Result<(), PlotError> {
    let root = SVGBackend::new(file_name, (2048, 1024)).into_drawing_area();
    root.fill(&WHITE)?;

    if all_time_series.len() == 0 {
        return Err(PlotError::EmptyTimeSeries);
    }
    let (mut min_date, mut max_date, mut min_val, mut max_val) = all_time_series[0].min_max()?;

    // Calculate max ranges over all time series
    for ts in &all_time_series[1..] {
        let (min_date_tmp, max_date_tmp, min_val_tmp, max_val_tmp) = ts.min_max()?;
        if min_date_tmp < min_date {
            min_date = min_date_tmp;
        }
        if max_date_tmp > max_date {
            max_date = max_date_tmp;
        }
        if min_val_tmp < min_val {
            min_val = min_val_tmp;
        }
        if max_val_tmp > max_val {
            max_val = max_val_tmp;
        }
    }

    let y_range = min_val..max_val;
    let min_time = make_time(min_date.year(), min_date.month(), 1, 0, 0, 0).unwrap();
    let max_year = max_date.year();
    let max_month = max_date.month();
    let max_time = make_time(
        max_year,
        max_month,
        last_day_of_month(max_year, max_month),
        23,
        59,
        59,
    )
    .unwrap();
    let x_range = (min_time..max_time).monthly();

    let mut chart = ChartBuilder::on(&root)
        .margin(10)
        .caption(title, ("sans-serif", 40))
        .set_label_area_size(LabelAreaPosition::Left, 80)
        .set_label_area_size(LabelAreaPosition::Bottom, 60)
        .build_cartesian_2d(x_range, y_range)?;

    chart
        .configure_mesh()
        .disable_x_mesh()
        .disable_y_mesh()
        .x_labels(30)
        .y_desc("Total position value (€)")
        .x_desc("Date")
        .label_style(("sans-serif", 16))
        .axis_desc_style(("sans-serif", 20))
        .draw()?;

    static COLORS: [&'static RGBColor; 5] = [&BLUE, &GREEN, &RED, &CYAN, &MAGENTA];
    let mut color_index: usize = 0;
    for ts in all_time_series {
        chart
            .draw_series(LineSeries::new(
                ts.series.iter().map(|v| (v.time, v.value)),
                COLORS[color_index],
            ))?
            .label(&ts.title)
            .legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], COLORS[color_index]));
        color_index = (color_index + 1) % COLORS.len();
    }

    chart
        .configure_series_labels()
        .border_style(&BLACK)
        .position(SeriesLabelPosition::UpperLeft)
        .label_font(("sans-serif", 20))
        .draw()?;

    Ok(())
}
