use chrono::DateTime;
use wasm_graph::func_plot::draw_bmp;

fn sample_data() -> (Vec<i64>, Vec<f32>) {
    let dates_str = vec![
        "2014-01-02T09:00:00+01:00",
        "2015-08-21T09:00:00+02:00",
        "2016-07-04T09:00:00+02:00",
        "2017-03-08T09:00:00+01:00",
        "2018-03-07T09:00:00+01:00",
        "2019-03-18T09:00:00+01:00",
        "2020-05-12T09:02:00+02:00",
        "2022-01-20T09:05:05+01:00",
        "2022-03-21T09:05:03+01:00",
    ];
    let values = vec![
        1.4,
        3.4,
        2.8,
        2.2,
        3.6,
        4.9,
        4.5,
        6.7,
        8.9,
        4.1,
        4.3,
        8.2,
        2.2,
        6.3,
        9.4,
        5.4,
        7.6,
        9.8,
    ];
    let dates = dates_str.into_iter()
        .map(|s| DateTime::parse_from_rfc3339(s).unwrap())
        .map(|t| t.timestamp_millis() )
        .collect();
    (dates, values)
}

fn main() {
    let (x_axis, y_axis) = sample_data();
    draw_bmp("sample.bmp", "sample chart", &x_axis, &y_axis, r#"["sample", "sample2"]"#).unwrap();
}
