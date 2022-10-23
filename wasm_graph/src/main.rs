use chrono::{Local, TimeZone};
use serde::Serialize;
use wasm_graph::func_plot::draw;

#[derive(Debug, Serialize)]
struct Graph {
    name: String,
    values: Vec<f64>,
}

#[derive(Serialize)]
pub struct Source {
    name: String,
    start_idx: usize,
}

fn sample_data() -> (Vec<i64>, Vec<f32>, String) {
    let dates = vec![
        Local.ymd(2022, 1, 1).and_hms_milli(0, 0, 0, 0),
        Local.ymd(2022, 2, 1).and_hms_milli(0, 0, 0, 0),
        Local.ymd(2022, 3, 1).and_hms_milli(0, 0, 0, 0),
    ];
    let graphs = vec![
        Graph {
            name: "one".to_string(),
            values: vec![1., 4., 2.2],
        },
        Graph {
            name: "two".to_string(),
            values: vec![3., 2., 5.],
        },
        Graph {
            name: "three".to_string(),
            values: vec![2., 4., 2.],
        },
    ];

    let mut times = Vec::new();
    let mut values = Vec::new();
    let mut names = Vec::new();
    let mut idx = 0;
    for graph in graphs {
        graph.values.iter().for_each(|v| values.push(*v as f32));
        dates
            .iter()
            .map(|t| t.timestamp_millis())
            .for_each(|t| times.push(t));
        names.push(Source {
            name: graph.name,
            start_idx: idx,
        });
        idx = values.len();
    }
    let names = serde_json::to_string(&names).unwrap();
    (times, values, names)
}

fn main() {
    let (x_axis, values, names) = sample_data();
    let _ = draw("sample.bmp", "sample chart", &x_axis, &values, &names).unwrap();
}
