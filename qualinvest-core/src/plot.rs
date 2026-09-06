use plotters::prelude::*;
use thiserror::Error;

use cal_calc::last_day_of_month;
use chrono::{DateTime, Utc};
use finql::datatypes::date_time_helper::make_offset_time;
use finql::time_series::TimeSeries;
use time::OffsetDateTime;

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

fn convert_to_utc(time: &OffsetDateTime) -> DateTime<Utc> {
    DateTime::from_timestamp_secs(time.unix_timestamp()).unwrap()
}

/// Generate an SVG plot as a string that can be displayed in a web browser
pub fn make_plot(title: &str, all_time_series: &[TimeSeries]) -> Result<String, PlotError> {
    let mut svg_string = String::new();
    {
        let root = SVGBackend::with_string(&mut svg_string, (2048, 1024)).into_drawing_area();
        root.fill(&WHITE)?;

        if all_time_series.is_empty() {
            return Err(PlotError::EmptyTimeSeries);
        }
        let (mut min_date, mut max_date, mut min_val, mut max_val) =
            all_time_series[0].min_max()?;

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
        let min_time =
            make_offset_time(min_date.year(), min_date.month() as u32, 1, 0, 0, 0).unwrap();
        let max_year = max_date.year();
        let max_month = max_date.month();
        let max_time = make_offset_time(
            max_year,
            max_month as u32,
            last_day_of_month(max_year, max_month as u8) as u32,
            23,
            59,
            59,
        )
        .unwrap();
        let min_naive_time = convert_to_utc(&min_time);
        let max_naive_time = convert_to_utc(&max_time);
        let x_range = (min_naive_time..max_naive_time).monthly();

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

        static COLORS: [&RGBColor; 5] = [&BLUE, &GREEN, &RED, &CYAN, &MAGENTA];
        let mut color_index: usize = 0;
        for ts in all_time_series {
            chart
                .draw_series(LineSeries::new(
                    ts.series.iter().map(|v| (convert_to_utc(&v.time), v.value)),
                    COLORS[color_index],
                ))?
                .label(&ts.title)
                .legend(move |(x, y)| {
                    PathElement::new(vec![(x, y), (x + 20, y)], COLORS[color_index])
                });
            color_index = (color_index + 1) % COLORS.len();
        }

        chart
            .configure_series_labels()
            .border_style(BLACK)
            .position(SeriesLabelPosition::UpperLeft)
            .label_font(("sans-serif", 20))
            .draw()?;

        // Ensure the drawing is complete before dropping the root
        root.present()?;
    } // End of scope - this drops the SVGBackend and flushes to string

    Ok(svg_string)
}
