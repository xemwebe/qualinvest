use chrono::{Local, TimeZone};
use wasm_graph::func_plot::draw;
use serde::Serialize;

#[derive(Debug,Serialize)]
struct Graph {
    name: String,
    values: Vec<f64>,
}

fn sample_data() -> (Vec<i64>, Vec<f32>, String) {
    let dates = vec![
        Local.ymd(2022, 1, 1).and_hms_milli(0, 0, 0, 0), 
        Local.ymd(2022, 2, 1).and_hms_milli(0, 0, 0, 0), 
        Local.ymd(2022, 3, 1).and_hms_milli(0, 0, 0, 0), 
    ];
    let graphs = vec![
        Graph{ name: "one".to_string(), values: vec![ 1., 4., 2.] },
        Graph{ name: "two".to_string(), values: vec![ 3., 2., 5.] },
        Graph{ name: "three".to_string(), values: vec![ 2., 4., 2.] },
    ];

    let dates = dates.into_iter()
        .map(|t| t.timestamp_millis() )
        .collect();
    let mut values = vec![];
    let mut names= Vec::<String>::new();
    for graph in graphs {
        graph.values.iter().for_each(|v| values.push(*v as f32));
        names.push(graph.name);
    }
    let names = serde_json::to_string(&names).unwrap();
    (dates, values, names)
}

fn main() {
    let (x_axis, values, names) = sample_data();
    let _ = draw("sample.bmp", "sample chart", &x_axis, &values, &names).unwrap();
}
