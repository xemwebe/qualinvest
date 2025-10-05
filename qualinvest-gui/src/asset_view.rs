use crate::assets::AssetView;
use leptos::prelude::*;

#[component]
pub fn AssetsTable(
    assets: Vec<AssetView>,
    selected_asset_id: ReadSignal<Option<i32>>,
    set_selected_asset_id: WriteSignal<Option<i32>>,
) -> impl IntoView {
    view! {
        <table class="table">
            <thead>
                <tr>
                    <th class="header-cell">"ID"</th>
                    <th class="header-cell">"Name"</th>
                    <th class="header-cell">"Class"</th>
                </tr>
            </thead>
            <tbody>
                <For
                    each=move || assets.clone()
                    key=|asset| asset.id
                    children=move |asset| {
                        let asset_id = asset.id;
                        let is_selected = move || selected_asset_id.get() == Some(asset_id);
                        view! {
                            <tr
                                class:selected=is_selected
                                on:click=move |_| {
                                    set_selected_asset_id.set(Some(asset_id));
                                }
                            >
                                <td class="cell">{asset.id}</td>
                                <td class="cell">{asset.name}</td>
                                <td class="cell">{asset.class}</td>
                            </tr>
                        }
                    }
                />
            </tbody>
        </table>
    }
}
