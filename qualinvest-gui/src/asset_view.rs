use crate::assets::AssetView;
use leptos::prelude::*;

#[component]
pub fn AssetsTable(
    assets: Vec<AssetView>,
    selected_asset_info: ReadSignal<Option<(i32, String)>>,
    set_selected_asset_info: WriteSignal<Option<(i32, String)>>,
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
                        let asset_name = asset.name.clone();
                        let is_selected = move || if let Some((id, _)) = selected_asset_info.get() { id == asset_id } else { false };
                        view! {
                            <tr
                                class:selected=is_selected
                                on:click=move |_| {
                                    set_selected_asset_info.set(Some((asset_id, asset_name.clone())));
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
