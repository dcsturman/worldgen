use itertools::Itertools;
use leptos::prelude::*;
use reactive_stores::{Field, OptionStoreExt, Store, StoreFieldIterator};

use crate::gas_giant::{GasGiant, GasGiantStoreFields};
use crate::system::{OrbitContent, OrbitContentStoreFields, StarOrbit, System, SystemStoreFields};
use crate::world::{Satellites, SatellitesStoreFields, World, WorldStoreFields};

#[component]
pub fn WorldList(#[prop(default = false)] is_companion: bool) -> impl IntoView {
    let primary = expect_context::<Store<System>>();

    view! {
        <table class="world-table">
            <thead>
                <tr>
                    <th class="table-entry">"Orbit"</th>
                    <th class="table-entry"></th>
                    <th class="table-entry">"Name"</th>
                    <th class="table-entry">"UPP"</th>
                    <th class="table-entry">"Remarks"</th>
                    <th class="table-entry">"Astro Data"</th>
                </tr>
            </thead>
            <tbody>
                <StarRow system=primary is_companion=is_companion />
                {move || [primary.secondary(), primary.tertiary()].into_iter().map(|companion| {
                    if let Some(companion) = companion.get() {
                        let companion = Store::new(*companion);
                        if companion.orbit().get() == StarOrbit::Primary {
                            // TODO: Get rid of this clone.
                            view! { <StarRow system=companion is_companion=true /> }
                                .into_any()
                        } else {
                            view! { <></> }.into_any()
                        }
                    } else {
                        view! { <></> }.into_any()
                    }
                }).collect::<Vec<_>>().into_view()}
                {move || {
                    (0..primary.orbit_slots().read().len())
                        .map(|index| {
                            primary
                                .orbit_slots()
                                .at_unkeyed(index)
                                .with(|body| match body {
                                    Some(OrbitContent::World(_world)) => {
                                        let my_field = primary
                                            .orbit_slots()
                                            .at_unkeyed(index)
                                            .unwrap()
                                            .world_0()
                                            .unwrap();
                                        view! { <WorldView world=my_field satellite=false /> }
                                            .into_any()
                                    }
                                    Some(OrbitContent::GasGiant(_gas_giant)) => {
                                        let my_field = primary
                                            .orbit_slots()
                                            .at_unkeyed(index)
                                            .unwrap()
                                            .gas_giant_0()
                                            .unwrap();
                                        view! { <GiantView world=my_field /> }.into_any()
                                    }
                                    Some(OrbitContent::Secondary) => {
                                        let secondary = Store::new(
                                            *primary.secondary().unwrap().get(),
                                        );
                                        // TODO: Get rid of this clone.
                                        view! { <StarRow system=secondary is_companion=false /> }
                                            .into_any()
                                    }
                                    Some(OrbitContent::Tertiary) => {
                                        let tertiary = Store::new(
                                            *primary.tertiary().unwrap().get(),
                                        );
                                        // TODO: Get rid of this clone.
                                        view! { <StarRow system=tertiary is_companion=false /> }
                                            .into_any()
                                    }
                                    _ => view! { <></> }.into_any(),
                                })
                        })
                        .collect::<Vec<_>>()
                        .into_view()
                }}
                {move || [primary.secondary(), primary.tertiary()].into_iter().map(|companion| {
                    if let Some(companion) = companion.get() {
                        let companion = Store::new(*companion);
                        if companion.orbit().get() == StarOrbit::Far {
                            // TODO: Get rid of this clone.
                            view! { <StarRow system=companion is_companion=false /> }
                                .into_any()
                        } else {
                            view! { <></> }.into_any()
                        }
                    } else {
                        view! { <></> }.into_any()
                    }
                }).collect::<Vec<_>>().into_view()}
            </tbody>
        </table>
    }
}

#[component]
pub fn StarRow(
    #[prop(into)] system: Field<System>,
    #[prop(default = false)] is_companion: bool,
) -> impl IntoView {
    view! {
        <tr>
            <td class="table-entry">
                {move || {
                    if is_companion {
                        "Companion".to_string()
                    } else {
                        match system.orbit().get() {
                            StarOrbit::Primary => "Primary".to_string(),
                            StarOrbit::Far => "Far".to_string(),
                            StarOrbit::System(orbit) => orbit.to_string(),
                        }
                    }
                }}
            </td>
            <td class="table-entry"></td>
            <td class="table-entry">{move || system.name().get()}</td>
            <td class="table-entry">{move || system.star().get().to_string()}</td>
        </tr>
    }
}

#[component]
pub fn WorldView(#[prop(into)] world: Field<World>, satellite: bool) -> impl IntoView {
    {
        view! {
            <tr>
                // Add an indent for satellite orbit number
                <Show when=move || satellite>{move || view! { <td></td> }}</Show>
                <td class="table-entry">{move || world.read().orbit.to_string()}</td>
                <Show when=move || !satellite>{move || view! { <td></td> }}</Show>
                <td class="table-entry">{move || world.read().name.clone()}</td>
                <td class="table-entry">{move || world.with(|world| world.to_upp())}</td>
                <td class="table-entry">
                    {move || {
                        world
                            .with(|world| {
                                [world.facilities_string(), world.trade_classes_string()]
                                    .iter()
                                    .filter(|s| !s.is_empty())
                                    .cloned()
                                    .intersperse("; ".to_string())
                                    .collect::<String>()
                            })
                    }}
                </td>
                <td class="table-entry">
                    {move || world.with(|world| world.astro_data.describe(world))}
                </td>
            </tr>
            <SatelliteView satellites=world.satellites() />
        }
        .into_any()
    }
}

#[component]
pub fn GiantView(#[prop(into)] world: Field<GasGiant>) -> impl IntoView {
    view! {
        <tr>
            <td class="table-entry">{move || world.read().orbit.to_string()}</td>
            <td class="table-entry"></td>
            <td class="table-entry">{move || world.read().name.clone()}</td>
            <td class="table-entry">{move || world.with(|world| format!("{}", world.size))}</td>
        </tr>
        <SatelliteView satellites=world.satellites() />
    }
}

#[component]
pub fn SatelliteView(#[prop(into)] satellites: Field<Satellites>) -> impl IntoView {
    view! {
        {move || {
            (0..satellites.sats().read().len())
                .map(|index| {
                    let satellite = satellites.sats().at_unkeyed(index);
                    view! { <WorldView world=satellite satellite=true /> }
                })
                .collect::<Vec<_>>()
                .into_view()
        }}
    }
}
