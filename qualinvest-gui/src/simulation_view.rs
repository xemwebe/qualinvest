use crate::assets::{get_assets, AssetView};
use crate::simulation::{run_strategies, DividendParam, StrategyParams};
use crate::ticker::{get_tickers, TickerFilter, TickerView};
use crate::time_range::{TimeRange, TimeRangeSelector};
use leptos::prelude::*;
use std::sync::Arc;

// ── helper types ──────────────────────────────────────────────────────────────

/// A dividend row together with its index in the dividends Vec, used as the
/// item type for the `<For>` component so we can avoid a turbofish inside view!
#[derive(Clone, PartialEq)]
struct IndexedDividend {
    idx: usize,
    date: String,
    amount: String,
}

// ── per-strategy form state ───────────────────────────────────────────────────

/// A flat, fully owned representation of one strategy entry in the form.
#[derive(Debug, Clone, PartialEq)]
pub struct StrategyEntry {
    /// Unique client-side key (incrementing counter).
    pub key: u32,
    /// "StaticInSingleStock" | "ReInvestInSingleStock"
    pub strategy_type: String,
    // -- shared fields --
    pub asset_id: Option<i32>,
    pub initial_position: f64,
    pub initial_cash: f64,
    pub currency: String,
    // transaction costs
    pub min_fee: f64,
    pub max_fee_enabled: bool,
    pub max_fee: f64,
    pub proportional_fee: f64,
    pub tax_rate: f64,
    // dividends: list of (date_str, amount_str) rows
    pub dividends: Vec<(String, String)>,
    // -- ReInvestInSingleStock only --
    pub ticker_id: Option<i32>,
}

impl StrategyEntry {
    fn new(key: u32) -> Self {
        Self {
            key,
            strategy_type: "StaticInSingleStock".to_string(),
            asset_id: None,
            initial_position: 0.0,
            initial_cash: 0.0,
            currency: "EUR".to_string(),
            min_fee: 0.0,
            max_fee_enabled: false,
            max_fee: 0.0,
            proportional_fee: 0.0,
            tax_rate: 0.0,
            dividends: vec![("".to_string(), "".to_string())],
            ticker_id: None,
        }
    }
}

// ── top-level page component ──────────────────────────────────────────────────

#[component]
pub fn SimulationPage() -> impl IntoView {
    let (next_key, set_next_key) = signal(1_u32);
    let (strategies, set_strategies) = signal(vec![StrategyEntry::new(0)]);
    let (selected_time_range, set_selected_time_range) = signal(TimeRange::All);

    // Incremented each time the user clicks "Start Simulation"; 0 means never run.
    let (run_trigger, set_run_trigger) = signal(0_u32);

    let add_strategy = move |_| {
        let key = next_key.get_untracked();
        set_next_key.set(key + 1);
        set_strategies.update(|v| v.push(StrategyEntry::new(key)));
    };

    // Convert StrategyEntry list to StrategyParams and call the server function.
    let simulation_result = Resource::new(
        move || run_trigger.get(),
        move |trigger| {
            let entries = strategies.get_untracked();
            let time_range = selected_time_range.get_untracked();
            async move {
                if trigger == 0 {
                    return None;
                }
                let params: Vec<StrategyParams> = entries
                    .into_iter()
                    .enumerate()
                    .filter_map(|(i, e)| {
                        let asset_id = e.asset_id?;
                        Some(StrategyParams {
                            label: format!("Strategy {i}: {}", e.strategy_type),
                            strategy_type: e.strategy_type,
                            asset_id,
                            ticker_id: e.ticker_id,
                            initial_position: e.initial_position,
                            initial_cash: e.initial_cash,
                            currency: e.currency,
                            min_fee: e.min_fee,
                            max_fee: if e.max_fee_enabled {
                                Some(e.max_fee)
                            } else {
                                None
                            },
                            proportional_fee: e.proportional_fee,
                            tax_rate: e.tax_rate,
                            dividends: e
                                .dividends
                                .into_iter()
                                .filter(|(d, a)| !d.is_empty() && !a.is_empty())
                                .filter_map(|(d, a)| {
                                    let amount = a.parse::<f64>().ok()?;
                                    Some(DividendParam { date: d, amount })
                                })
                                .collect(),
                        })
                    })
                    .collect();
                Some(run_strategies(params, time_range).await)
            }
        },
    );

    view! {
        <div class="simulation-strategies">
            <Suspense fallback=|| view! { <p>"Loading assets…"</p> }>
                <Await future=get_assets() let:assets_result>
                    {
                        let assets: Vec<AssetView> = assets_result
                            .as_ref()
                            .map(|s| s.get())
                            .unwrap_or_default();

                        view! {
                            <For
                                each=move || strategies.get()
                                key=|s| s.key
                                children={
                                    let assets = assets.clone();
                                    move |entry| {
                                        let key = entry.key;
                                        let assets = assets.clone();
                                        view! {
                                            <StrategyCard
                                                entry=entry
                                                assets=assets
                                                on_change=move |updated| {
                                                    set_strategies.update(|v| {
                                                        if let Some(slot) = v.iter_mut().find(|e| e.key == key) {
                                                            *slot = updated;
                                                        }
                                                    });
                                                }
                                                on_remove=move || {
                                                    set_strategies.update(|v| v.retain(|e| e.key != key));
                                                }
                                            />
                                        }
                                    }
                                }
                            />
                        }
                    }
                </Await>
            </Suspense>

            <button class="button" on:click=add_strategy>
                "+ Add Strategy"
            </button>
            <div class="divider"></div>
            <div class="simulation-run-section">
                <h2>"Simulate strategies"</h2>
                <div class="time-range-wrapper">
                    <TimeRangeSelector set_selected=set_selected_time_range />
                </div>
                <button
                    class="button"
                    on:click=move |_| set_run_trigger.update(|n| *n += 1)
                >
                    "Start Simulation"
                </button>
            </div>

            <Suspense fallback=|| view! { <p class="simulation-running">"Running simulation…"</p> }>
                {move || {
                    simulation_result.get().flatten().map(|outcome| match outcome {
                        Err(e) => view! {
                            <p class="error">"Simulation failed: " {e.to_string()}</p>
                        }.into_any(),
                        Ok(svg) => view! {
                            <div class="simulation-graph" inner_html=svg />
                        }.into_any(),
                    })
                }}
            </Suspense>
        </div>
    }
}

// ── single strategy card ──────────────────────────────────────────────────────

#[component]
fn StrategyCard<FChange, FRemove>(
    entry: StrategyEntry,
    assets: Vec<AssetView>,
    on_change: FChange,
    on_remove: FRemove,
) -> impl IntoView
where
    FChange: Fn(StrategyEntry) + 'static + Clone + Send + Sync,
    FRemove: Fn() + 'static + Send + Sync,
{
    let (strategy_type, set_strategy_type) = signal(entry.strategy_type.clone());
    let (asset_id, set_asset_id) = signal(entry.asset_id);
    let (initial_position, set_initial_position) = signal(entry.initial_position);
    let (initial_cash, set_initial_cash) = signal(entry.initial_cash);
    let (currency, set_currency) = signal(entry.currency.clone());
    let (min_fee, set_min_fee) = signal(entry.min_fee);
    let (max_fee_enabled, set_max_fee_enabled) = signal(entry.max_fee_enabled);
    let (max_fee, set_max_fee) = signal(entry.max_fee);
    let (proportional_fee, set_proportional_fee) = signal(entry.proportional_fee);
    let (tax_rate, set_tax_rate) = signal(entry.tax_rate);
    let (dividends, set_dividends) = signal(entry.dividends.clone());
    let (ticker_id, set_ticker_id) = signal(entry.ticker_id);

    let key = entry.key;

    // Collect current state and notify the parent.
    // Wrapped in Arc so it can be cheaply cloned into many closures.
    let emit: Arc<dyn Fn() + Send + Sync> = Arc::new({
        let on_change = on_change.clone();
        move || {
            on_change(StrategyEntry {
                key,
                strategy_type: strategy_type.get_untracked(),
                asset_id: asset_id.get_untracked(),
                initial_position: initial_position.get_untracked(),
                initial_cash: initial_cash.get_untracked(),
                currency: currency.get_untracked(),
                min_fee: min_fee.get_untracked(),
                max_fee_enabled: max_fee_enabled.get_untracked(),
                max_fee: max_fee.get_untracked(),
                proportional_fee: proportional_fee.get_untracked(),
                tax_rate: tax_rate.get_untracked(),
                dividends: dividends.get_untracked(),
                ticker_id: ticker_id.get_untracked(),
            });
        }
    });

    // Tickers are only fetched when the type is ReInvestInSingleStock and an
    // asset has been selected.
    let ticker_resource = Resource::new(
        move || (strategy_type.get(), asset_id.get()),
        move |(stype, aid)| async move {
            if stype == "ReInvestInSingleStock" {
                if let Some(id) = aid {
                    return get_tickers(TickerFilter { asset_id: id })
                        .await
                        .map(|s| s.get_untracked())
                        .unwrap_or_default();
                }
            }
            Vec::<TickerView>::new()
        },
    );

    let assets_for_select = assets.clone();

    // Pre-clone emit for each move closure that consumes it inside view!
    let emit_type = emit.clone();
    let emit_max_fee = emit.clone();
    let emit_dividends_for = emit.clone();
    let emit_add_dividend = emit.clone();

    view! {
        <div class="strategy-card">

            // ── header: type selector + remove button ─────────────────────────
            <div class="strategy-card-header">
                <div class="form-group">
                    <label>"Strategy Type"</label>
                    <select
                        prop:value=move || strategy_type.get()
                        on:change={
                            let emit = emit.clone();
                            move |ev| {
                                set_strategy_type.set(event_target_value(&ev));
                                set_ticker_id.set(None);
                                emit();
                            }
                        }
                    >
                        <option value="StaticInSingleStock">"StaticInSingleStock"</option>
                        <option value="ReInvestInSingleStock">"ReInvestInSingleStock"</option>
                    </select>
                </div>
                <button
                    class="button strategy-remove-btn"
                    on:click=move |_| on_remove()
                >
                    "Remove"
                </button>
            </div>

            // ── asset selector ────────────────────────────────────────────────
            <div class="form-group">
                <label>"Asset"</label>
                <select
                    prop:value=move || asset_id.get().map(|id| id.to_string()).unwrap_or_default()
                    on:change={
                        let emit = emit.clone();
                        move |ev| {
                            let val = event_target_value(&ev);
                            set_asset_id.set(val.parse::<i32>().ok());
                            set_ticker_id.set(None);
                            emit();
                        }
                    }
                >
                    <option value="">"— select asset —"</option>
                    {assets_for_select
                        .iter()
                        .map(|a| {
                            let id_str = a.id.to_string();
                            let name = a.name.clone();
                            view! { <option value=id_str>{name}</option> }
                        })
                        .collect::<Vec<_>>()}
                </select>
            </div>

            // ── ticker (ReInvestInSingleStock only) ───────────────────────────
            {move || {
                if strategy_type.get() == "ReInvestInSingleStock" {
                    let emit = emit_type.clone();
                    view! {
                        <div class="form-group">
                            <label>"Ticker"</label>
                            <Suspense fallback=|| view! { <span>"Loading tickers…"</span> }>
                                {move || {
                                    let tickers = ticker_resource.get().unwrap_or_default();
                                    let emit = emit.clone();
                                    view! {
                                        <select
                                            prop:value=move || {
                                                ticker_id.get()
                                                    .map(|id| id.to_string())
                                                    .unwrap_or_default()
                                            }
                                            on:change={
                                                let emit = emit.clone();
                                                move |ev| {
                                                    let val = event_target_value(&ev);
                                                    set_ticker_id.set(val.parse::<i32>().ok());
                                                    emit();
                                                }
                                            }
                                        >
                                            <option value="">"— select ticker —"</option>
                                            {tickers
                                                .iter()
                                                .map(|t| {
                                                    let id_str = t.id.to_string();
                                                    let name = t.name.clone();
                                                    view! { <option value=id_str>{name}</option> }
                                                })
                                                .collect::<Vec<_>>()}
                                        </select>
                                    }
                                }}
                            </Suspense>
                        </div>
                    }.into_any()
                } else {
                    ().into_any()
                }
            }}

            // ── initial position, cash, currency ──────────────────────────────
            <div class="strategy-row">
                <div class="form-group">
                    <label>"Initial Position"</label>
                    <input
                        type="number"
                        step="any"
                        min="0"
                        prop:value=move || initial_position.get().to_string()
                        on:input={
                            let emit = emit.clone();
                            move |ev| {
                                if let Ok(v) = event_target_value(&ev).parse::<f64>() {
                                    set_initial_position.set(v);
                                    emit();
                                }
                            }
                        }
                    />
                </div>
                <div class="form-group">
                    <label>"Initial Cash"</label>
                    <input
                        type="number"
                        step="any"
                        prop:value=move || initial_cash.get().to_string()
                        on:input={
                            let emit = emit.clone();
                            move |ev| {
                                if let Ok(v) = event_target_value(&ev).parse::<f64>() {
                                    set_initial_cash.set(v);
                                    emit();
                                }
                            }
                        }
                    />
                </div>
                <div class="form-group">
                    <label>"Currency"</label>
                    <input
                        type="text"
                        maxlength="3"
                        placeholder="EUR"
                        prop:value=move || currency.get()
                        on:input={
                            let emit = emit.clone();
                            move |ev| {
                                set_currency.set(event_target_value(&ev).to_uppercase());
                                emit();
                            }
                        }
                    />
                </div>
            </div>

            // ── transaction costs ─────────────────────────────────────────────
            <fieldset class="strategy-fieldset">
                <legend>"Transaction Costs"</legend>

                <div class="strategy-row">
                    <div class="form-group">
                        <label>"Min Fee"</label>
                        <input
                            type="number"
                            step="any"
                            min="0"
                            prop:value=move || min_fee.get().to_string()
                            on:input={
                                let emit = emit.clone();
                                move |ev| {
                                    if let Ok(v) = event_target_value(&ev).parse::<f64>() {
                                        set_min_fee.set(v);
                                        emit();
                                    }
                                }
                            }
                        />
                    </div>
                    <div class="form-group">
                        <label>"Proportional Fee (%)"</label>
                        <input
                            type="number"
                            step="0.0001"
                            min="0"
                            max="100"
                            prop:value=move || format!("{:.4}", proportional_fee.get() * 100.0)
                            on:input={
                                let emit = emit.clone();
                                move |ev| {
                                    if let Ok(v) = event_target_value(&ev).parse::<f64>() {
                                        set_proportional_fee.set(v / 100.0);
                                        emit();
                                    }
                                }
                            }
                        />
                    </div>
                    <div class="form-group">
                        <label>"Tax Rate (%)"</label>
                        <input
                            type="number"
                            step="0.0001"
                            min="0"
                            max="100"
                            prop:value=move || format!("{:.4}", tax_rate.get() * 100.0)
                            on:input={
                                let emit = emit.clone();
                                move |ev| {
                                    if let Ok(v) = event_target_value(&ev).parse::<f64>() {
                                        set_tax_rate.set(v / 100.0);
                                        emit();
                                    }
                                }
                            }
                        />
                    </div>
                </div>

                // Max fee (optional) ------------------------------------------
                <div class="strategy-row">
                    <div class="form-group form-group--inline">
                        <label>
                            <input
                                type="checkbox"
                                prop:checked=move || max_fee_enabled.get()
                                on:change={
                                    let emit = emit.clone();
                                    move |ev| {
                                        use leptos::wasm_bindgen::JsCast;
                                        let checked = ev
                                            .target()
                                            .and_then(|t| {
                                                t.dyn_into::<web_sys::HtmlInputElement>().ok()
                                            })
                                            .map(|el| el.checked())
                                            .unwrap_or(false);
                                        set_max_fee_enabled.set(checked);
                                        emit();
                                    }
                                }
                            />
                            " Enable Max Fee"
                        </label>
                        {move || {
                            if max_fee_enabled.get() {
                                let emit = emit_max_fee.clone();
                                view! {
                                    <input
                                        type="number"
                                        step="any"
                                        min="0"
                                        prop:value=move || max_fee.get().to_string()
                                        on:input={
                                            let emit = emit.clone();
                                            move |ev| {
                                                if let Ok(v) = event_target_value(&ev).parse::<f64>() {
                                                    set_max_fee.set(v);
                                                    emit();
                                                }
                                            }
                                        }
                                    />
                                }.into_any()
                            } else {
                                view! {
                                    <span class="disabled-note">"(no maximum)"</span>
                                }.into_any()
                            }
                        }}
                    </div>
                </div>
            </fieldset>

            // ── dividends ─────────────────────────────────────────────────────
            <fieldset class="strategy-fieldset">
                <legend>"Dividends"</legend>
                <div class="dividends-header">
                    <span class="dividend-col-label">"Date"</span>
                    <span class="dividend-col-label">"Amount per Share"</span>
                </div>

                <For
                    each=move || {
                        dividends
                            .get()
                            .into_iter()
                            .enumerate()
                            .map(|(idx, (date, amount))| IndexedDividend { idx, date, amount })
                            .collect::<Vec<IndexedDividend>>()
                    }
                    key=|item| item.idx
                    children={
                        let emit = emit_dividends_for.clone();
                        move |item| {
                            let idx = item.idx;
                            let emit = emit.clone();
                            let (local_date, set_local_date) = signal(item.date);
                            let (local_amount, set_local_amount) = signal(item.amount);

                            view! {
                                <div class="dividend-row">
                                    <input
                                        type="date"
                                        prop:value=move || local_date.get()
                                        on:input={
                                            let emit = emit.clone();
                                            move |ev| {
                                                let v = event_target_value(&ev);
                                                set_local_date.set(v.clone());
                                                set_dividends.update(|rows| {
                                                    if let Some(row) = rows.get_mut(idx) {
                                                        row.0 = v;
                                                    }
                                                });
                                                emit();
                                            }
                                        }
                                    />
                                    <input
                                        type="number"
                                        step="any"
                                        min="0"
                                        placeholder="0.00"
                                        prop:value=move || local_amount.get()
                                        on:input={
                                            let emit = emit.clone();
                                            move |ev| {
                                                let v = event_target_value(&ev);
                                                set_local_amount.set(v.clone());
                                                set_dividends.update(|rows| {
                                                    if let Some(row) = rows.get_mut(idx) {
                                                        row.1 = v;
                                                    }
                                                });
                                                emit();
                                            }
                                        }
                                    />
                                    <button
                                        class="button"
                                        on:click={
                                            let emit = emit.clone();
                                            move |_| {
                                                set_dividends.update(|rows| {
                                                    if rows.len() > 1 {
                                                        rows.remove(idx);
                                                    }
                                                });
                                                emit();
                                            }
                                        }
                                    >
                                        "−"
                                    </button>
                                </div>
                            }
                        }
                    }
                />

                <button
                    class="button"
                    on:click={
                        let emit = emit_add_dividend.clone();
                        move |_| {
                            set_dividends.update(|rows| {
                                rows.push(("".to_string(), "".to_string()));
                            });
                            emit();
                        }
                    }
                >
                    "+ Add Dividend"
                </button>
            </fieldset>

        </div>
    }
}
