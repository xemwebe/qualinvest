use leptos::prelude::*;
use time::{macros::format_description, Date};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeRange {
    All,
    Latest,
    Custom(CustomTimeRange),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CustomTimeRange {
    pub start: TimeRangePoint,
    pub end: TimeRangePoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeRangePoint {
    Inception,
    Today,
    Custom(Date),
}

#[component]
pub fn TimeRangeSelector(
    selected: ReadSignal<TimeRange>,
    set_selected: WriteSignal<TimeRange>,
) -> impl IntoView {
    let (range_type, set_range_type) = signal("All".to_string());
    let (start_point, set_start_point) = signal("Inception".to_string());
    let (end_point, set_end_point) = signal("Today".to_string());
    let (start_date, set_start_date) = signal(String::new());
    let (end_date, set_end_date) = signal(String::new());

    // Update the selected TimeRange when any input changes
    let update_time_range = move || {
        let range = match range_type.get().as_str() {
            "All" => TimeRange::All,
            "Latest" => TimeRange::Latest,
            "Custom" => {
                let date_format = format_description!("[year]-[month]-[day]");

                let start = match start_point.get().as_str() {
                    "Inception" => TimeRangePoint::Inception,
                    "Today" => TimeRangePoint::Today,
                    "Custom" => {
                        if let Ok(date) = Date::parse(&start_date.get(), &date_format) {
                            TimeRangePoint::Custom(date)
                        } else {
                            TimeRangePoint::Inception
                        }
                    }
                    _ => TimeRangePoint::Inception,
                };

                let end = match end_point.get().as_str() {
                    "Inception" => TimeRangePoint::Inception,
                    "Today" => TimeRangePoint::Today,
                    "Custom" => {
                        if let Ok(date) = Date::parse(&end_date.get(), &date_format) {
                            TimeRangePoint::Custom(date)
                        } else {
                            TimeRangePoint::Today
                        }
                    }
                    _ => TimeRangePoint::Today,
                };

                TimeRange::Custom(CustomTimeRange { start, end })
            }
            _ => TimeRange::All,
        };
        set_selected.set(range);
    };

    view! {
        <div class="time-range-selector">
            <div class="form-group">
                <label for="range-type">"Time Range Type: "</label>
                <select
                    id="range-type"
                    on:change=move |ev| {
                        set_range_type.set(event_target_value(&ev));
                        update_time_range();
                    }
                    prop:value=move || range_type.get()
                >
                    <option value="Latest">"Latest"</option>
                    <option value="Custom">"Custom"</option>
                    <option value="All">"All"</option>
                </select>
            </div>

            {move || {
                if range_type.get() == "Custom" {
                    view! {
                        <div class="custom-range-controls">
                            <div class="form-group">
                                <label for="start-point">"Start: "</label>
                                <select
                                    id="start-point"
                                    on:change=move |ev| {
                                        set_start_point.set(event_target_value(&ev));
                                        update_time_range();
                                    }
                                    prop:value=move || start_point.get()
                                >
                                    <option value="Inception">"Inception"</option>
                                    <option value="Custom">"Custom Date"</option>
                                </select>

                                {move || {
                                    if start_point.get() == "Custom" {
                                        view! {
                                            <input
                                                type="date"
                                                on:input=move |ev| {
                                                    set_start_date.set(event_target_value(&ev));
                                                    update_time_range();
                                                }
                                                prop:value=move || start_date.get()
                                            />
                                        }.into_any()
                                    } else {
                                        view! { <></> }.into_any()
                                    }
                                }}
                            </div>

                            <div class="form-group">
                                <label for="end-point">"End: "</label>
                                <select
                                    id="end-point"
                                    on:change=move |ev| {
                                        set_end_point.set(event_target_value(&ev));
                                        update_time_range();
                                    }
                                    prop:value=move || end_point.get()
                                >
                                    <option value="Today">"Today"</option>
                                    <option value="Custom">"Custom Date"</option>
                                </select>

                                {move || {
                                    if end_point.get() == "Custom" {
                                        view! {
                                            <input
                                                type="date"
                                                on:input=move |ev| {
                                                    set_end_date.set(event_target_value(&ev));
                                                    update_time_range();
                                                }
                                                prop:value=move || end_date.get()
                                            />
                                        }.into_any()
                                    } else {
                                        view! { <></> }.into_any()
                                    }
                                }}
                            </div>
                        </div>
                    }.into_any()
                } else {
                    view! { <></> }.into_any()
                }
            }}
        </div>
    }
}
