use leptos::prelude::*;
use reactive_stores::{Field, OptionStoreExt, Store, StoreFieldIterator};
use itertools::Itertools;

use crate::worldgen::{
    GasGiant, GasGiantStoreFields, OrbitContent, OrbitContentStoreFields, Satellites,
    SatellitesStoreFields, StarOrbit, System, SystemStoreFields, World, WorldStoreFields,
};

#[component]
pub fn WorldTable() -> impl IntoView {
    let primary = expect_context::<Store<System>>();

    view! {
        <table class="world-table">
            <thead>
                <tr>
                    <th>"Orbit"</th>
                    <th></th>
                    <th>"Name"</th>
                    <th>"UPP"</th>
                    <th>"Remarks"</th>
                    <th>"Astro Data"</th>
                </tr>
            </thead>
            <tbody>
                <StarRow system=primary />
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
                                        // Terrible that we have to clone here!
                                        view! { <StarRow system=secondary /> }
                                            .into_any()
                                    }
                                    Some(OrbitContent::Tertiary) => {
                                        let tertiary = Store::new(
                                            *primary.tertiary().unwrap().get(),
                                        );
                                        // Terrible that we have to clone here!
                                        view! { <StarRow system=tertiary /> }
                                            .into_any()
                                    }
                                    _ => view! { <></> }.into_any(),
                                })
                        })
                        .collect::<Vec<_>>()
                        .into_view()
                }}
            </tbody>
        </table>
    }
}

#[component]
pub fn StarRow(#[prop(into)] system: Field<System>) -> impl IntoView {
    view! {
        <tr>
            <td>
                {move || match system.orbit().get() {
                    StarOrbit::Primary => "Primary".to_string(),
                    StarOrbit::Far => "Far".to_string(),
                    StarOrbit::System(orbit) => orbit.to_string(),
                }}
            </td>
            <td></td>
            <td>{move || system.name().get()}</td>
            <td>{move || system.star().get().to_string()}</td>
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
                <td>{move || world.read().orbit.to_string()}</td>
                <Show when=move || !satellite>{move || view! { <td></td> }}</Show>
                <td>{move || world.read().name.clone()}</td>
                <td>{move || world.with(|world| world.to_upp())}</td>
                <td>
                    {move || {
                        world
                            .with(|world| {
                                vec![world.facilities_string(), world.trade_classes_string()]
                                    .iter()
                                    .filter(|s| s.len() > 0)
                                    .cloned()
                                    .intersperse("; ".to_string())
                                    .collect::<String>()
                            })
                    }}
                </td>
                <td>{move || world.with(|world| world.astro_data.describe(world))}</td>
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
            <td>{move || world.read().orbit.to_string()}</td>
            <td></td>
            <td>{move || world.read().name.clone()}</td>
            <td>{move || world.with(|world| format!("{}", world.size))}</td>
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
