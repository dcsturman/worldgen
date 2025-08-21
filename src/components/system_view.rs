use leptos::context::Provider;
use leptos::prelude::*;
use reactive_stores::{Store, Subfield};

use crate::components::world_list::WorldList;
use crate::systems::has_satellites::HasSatellites;
use crate::systems::system::{OrbitContent, StarOrbit, System, SystemStoreFields};
use crate::systems::system_tables::get_habitable;
use crate::systems::world::World;

fn habitable_clause(system: &System) -> String {
    let habitable = get_habitable(&system.star);
    if habitable > -1 && habitable <= system.get_max_orbits() as i32 {
        format!(" with a habitable zone at orbit {habitable}")
    } else {
        " with no habitable zone".to_string()
    }
}

#[component]
pub fn SystemView() -> impl IntoView {
    let primary = expect_context::<Store<System>>();
    let main_world = expect_context::<Store<World>>();
    view! {
        <div class="output-region">
            <h2>"The " {move || main_world.read().name.clone()} " System"</h2>
            "The primary star of the "
            {move || main_world.read().name.clone()}
            " system is "
            <b>{move || primary.name().get()}</b>
            ", a "
            {move || primary.star().get().to_string()}
            " star"
            {move || habitable_clause(&primary.read())}
            ". "
            <SystemPreamble />
            <br />
            <br />
            <SystemMain />
        </div>
    }
}

fn lead_builder(
    system: Subfield<Store<System>, System, Option<Box<System>>>,
    kind: &str,
) -> impl '_ + Fn() -> String {
    move || {
        system.with(|subsystem| {
            if let Some(subsystem) = subsystem {
                match subsystem.orbit {
                    StarOrbit::Primary => {
                        format!(
                            " It has a {} contact star {}, which is a {} star.",
                            kind,
                            subsystem.name.clone(),
                            subsystem.star
                        )
                    }

                    StarOrbit::Far => {
                        format!(
                            " It has a {} star {} in far orbit, which is a {} star{}.",
                            kind,
                            subsystem.name.clone(),
                            subsystem.star,
                            habitable_clause(subsystem)
                        )
                    }
                    StarOrbit::System(orbit) => {
                        format!(
                            " It has a {} star {} at orbit {}, which is a {} star{}.",
                            kind,
                            subsystem.name.clone(),
                            orbit,
                            subsystem.star,
                            habitable_clause(subsystem)
                        )
                    }
                }
            } else {
                "".to_string()
            }
        })
    }
}

fn quantity_suffix(quantity: usize, singular: &str) -> String {
    if quantity == 0 {
        "".to_string()
    } else if quantity == 1 {
        format!("1 {singular}")
    } else {
        format!("{quantity} {singular}s")
    }
}

#[component]
pub fn SystemPreamble() -> impl IntoView {
    let system = expect_context::<Store<System>>();
    let main_world = expect_context::<Store<World>>();

    let secondary_lead = lead_builder(system.secondary(), "secondary");
    let tertiary_lead = lead_builder(system.tertiary(), "tertiary");

    let num_stars = move || system.read().count_stars() as usize - 1;
    let num_gas_giants = move || {
        system
            .read()
            .orbit_slots
            .iter()
            .filter(|&body| matches!(&body, Some(OrbitContent::GasGiant(_))))
            .count()
    };
    let num_planetoids = move || {
        system.read().orbit_slots.iter().filter(|&body| matches!(&body, Some(OrbitContent::World(world)) if world.name == "Planetoid Belt")).count()
    };
    let num_satellites = move || {
        system
            .read()
            .orbit_slots
            .iter()
            .filter_map(|body| match body {
                Some(OrbitContent::World(world)) => Some(world.get_num_satellites()),
                Some(OrbitContent::GasGiant(gas_giant)) => Some(gas_giant.get_num_satellites()),
                _ => None,
            })
            .sum::<usize>()
    };

    view! {
        <span>
            <span>
                <Show when=move || {
                    num_gas_giants() + num_stars() + num_planetoids() + num_satellites() > 0
                }>
                    {move || {
                        view! {
                            {main_world.read().name.clone()}
                            " has "
                            {move || {
                                itertools::Itertools::intersperse(
                                        [
                                            quantity_suffix(num_stars(), "star"),
                                            {
                                                if num_gas_giants() >= 2 {
                                                    format!("{} gas giants", num_gas_giants())
                                                } else if num_gas_giants() == 1 {
                                                    "1 gas giant".to_string()
                                                } else {
                                                    "".to_string()
                                                }
                                            },
                                            {
                                                if num_planetoids() >= 2 {
                                                    format!("{} planetoids", num_planetoids())
                                                } else if num_planetoids() == 1 {
                                                    "1 planetoid".to_string()
                                                } else {
                                                    "".to_string()
                                                }
                                            },
                                            {
                                                if num_satellites() >= 2 {
                                                    format!("{} satellites", num_satellites())
                                                } else if num_satellites() == 1 {
                                                    "1 satellite".to_string()
                                                } else {
                                                    "".to_string()
                                                }
                                            },
                                        ]
                                            .iter()
                                            .filter(|x| !x.is_empty())
                                            .cloned(),
                                        ", ".to_string(),
                                    )
                                    .collect::<String>()
                            }}
                        }
                    }} "."
                </Show>
            </span>
            {secondary_lead}

            {tertiary_lead}
        </span>
    }
}

#[component]
pub fn SystemMain(#[prop(default = false)] is_companion: bool) -> impl IntoView {
    let system = expect_context::<Store<System>>();

    view! {
        <div>
            <WorldList is_companion=is_companion />
            <br />
            {move || {
                if let Some(secondary) = system.secondary().get() {
                    let secondary = Store::new(*secondary);
                    view! {
                        {system.read().name.clone()}
                        "'s secondary star "
                        {secondary.name().get()}
                        :
                        <br />
                        <Provider value=secondary>
                            <SystemMain is_companion=true />
                        </Provider>
                        <br />
                    }
                        .into_any()
                } else {
                    ().into_any()
                }
            }}
            {move || {
                if let Some(tertiary) = system.tertiary().get() {
                    let tertiary = Store::new(*tertiary);
                    view! {
                        {system.read().name.clone()}
                        "'s tertiary star "
                        {tertiary.name().get()}
                        :
                        <br />
                        <Provider value=tertiary>
                            <SystemMain is_companion=true />
                        </Provider>
                        <br />
                    }
                        .into_any()
                } else {
                    ().into_any()
                }
            }}
        </div>
    }
}
