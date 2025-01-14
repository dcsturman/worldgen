use crate::system::{Star, StarSize, StarSubType, StarType};
use crate::util::roll_2d6;
use lazy_static::lazy_static;
use log::debug;
use std::collections::HashMap;

pub fn get_zone(star: &Star) -> ZoneTable {
    debug!(
        "get_zone: {:?} as {:?}",
        star,
        ZONE_TABLE.get(&(star.size, star.star_type, round_subtype(star.subtype)))
    );
    *ZONE_TABLE
        .get(&(star.size, star.star_type, round_subtype(star.subtype)))
        .unwrap()
}

pub fn get_habitable(star: &Star) -> i32 {
    let habitable = get_zone(star).habitable;
    if habitable > get_zone(star).inner {
        habitable
    } else {
        -1
    }
}

pub fn round_subtype(subtype: StarSubType) -> u8 {
    match subtype {
        0..=4 => 0,
        5..=9 => 5,
        _ => panic!("Invalid subtype"),
    }
}

pub(crate) fn get_luminosity(star: &Star) -> f32 {
    *LUMINOSITY_TABLE
        .get(&(star.star_type, round_subtype(star.subtype), star.size))
        .unwrap()
}

pub(crate) fn get_solar_mass(star: &Star) -> f32 {
    *MASS_TABLE
        .get(&(star.star_type, round_subtype(star.subtype), star.size))
        .unwrap()
}

pub(crate) fn get_orbital_distance(orbit: i32) -> f32 {
    ORBITAL_DISTANCE[orbit as usize]
}

pub(crate) fn get_cloudiness(atmosphere: i32) -> i32 {
    CLOUDINESS[atmosphere as usize]
}

pub(crate) fn get_greenhouse(atmosphere: i32) -> f32 {
    GREENHOUSE[atmosphere as usize]
}

pub(crate) fn get_world_temp(modifier: i32) -> f32 {
    let roll = (roll_2d6() + modifier).clamp(0, AVG_WORLD_TEMP.len() as i32 - 1) as usize;
    AVG_WORLD_TEMP[roll]
}

const ORBITAL_DISTANCE: [f32; 20] = [
    29.9, 59.8, 104.7, 149.6, 239.3, 418.9, 777.9, 1495.9, 2932.0, 5804.0, 11548.0, 23038.0,
    46016.0, 91972.0, 183885.0, 367711.0, 735363.0, 1470666.0, 2941274.0, 5882488.0,
];

const CLOUDINESS: [i32; 11] = [0, 0, 10, 10, 20, 30, 40, 50, 60, 70, 70];

const GREENHOUSE: [f32; 16] = [
    0.0, 0.0, 0.0, 0.0, 0.05, 0.05, 0.1, 0.1, 0.15, 0.15, 0.5, 0.5, 0.5, 0.15, 0.10, 0.0,
];

const AVG_WORLD_TEMP: [f32; 16] = [
    -2.5, 0.0, 2.5, 5.0, 7.5, 10.0, 12.5, 15.0, 17.5, 20.0, 22.5, 25.0, 27.5, 30.0, 32.5, 35.0,
];

#[derive(Debug, Clone, Copy)]
pub(crate) struct ZoneTable {
    pub(crate) inside: i32,
    pub(crate) hot: i32,
    pub(crate) inner: i32,
    pub(crate) habitable: i32,
    // For completeness we have outer, but its not currently used.
    #[allow(dead_code)]
    pub(crate) outer: i32,
}

lazy_static! {
    static ref ZONE_TABLE: HashMap<(StarSize, StarType, u8), ZoneTable> = HashMap::from_iter(vec![
        (
            (StarSize::Ia, StarType::O, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::Ia, StarType::O, 5),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::Ia, StarType::B, 0),
            ZoneTable {
                inside: 0,
                hot: 7,
                inner: 12,
                habitable: 13,
                outer: 14
            }
        ),
        (
            (StarSize::Ia, StarType::B, 5),
            ZoneTable {
                inside: 0,
                hot: 7,
                inner: 12,
                habitable: 13,
                outer: 14
            }
        ),
        (
            (StarSize::Ia, StarType::A, 0),
            ZoneTable {
                inside: 1,
                hot: 6,
                inner: 11,
                habitable: 12,
                outer: 14
            }
        ),
        (
            (StarSize::Ia, StarType::A, 5),
            ZoneTable {
                inside: 1,
                hot: 6,
                inner: 11,
                habitable: 12,
                outer: 14
            }
        ),
        (
            (StarSize::Ia, StarType::F, 0),
            ZoneTable {
                inside: 2,
                hot: 5,
                inner: 11,
                habitable: 12,
                outer: 14
            }
        ),
        (
            (StarSize::Ia, StarType::F, 5),
            ZoneTable {
                inside: 2,
                hot: 5,
                inner: 10,
                habitable: 11,
                outer: 14
            }
        ),
        (
            (StarSize::Ia, StarType::G, 0),
            ZoneTable {
                inside: 3,
                hot: 6,
                inner: 11,
                habitable: 12,
                outer: 14
            }
        ),
        (
            (StarSize::Ia, StarType::G, 5),
            ZoneTable {
                inside: 4,
                hot: 6,
                inner: 11,
                habitable: 12,
                outer: 14
            }
        ),
        (
            (StarSize::Ia, StarType::K, 0),
            ZoneTable {
                inside: 5,
                hot: 6,
                inner: 11,
                habitable: 12,
                outer: 14
            }
        ),
        (
            (StarSize::Ia, StarType::K, 5),
            ZoneTable {
                inside: 5,
                hot: 6,
                inner: 11,
                habitable: 12,
                outer: 14
            }
        ),
        (
            (StarSize::Ia, StarType::M, 0),
            ZoneTable {
                inside: 6,
                hot: 6,
                inner: 11,
                habitable: 12,
                outer: 14
            }
        ),
        (
            (StarSize::Ia, StarType::M, 5),
            ZoneTable {
                inside: 0,
                hot: 6,
                inner: 11,
                habitable: 12,
                outer: 14
            }
        ),
        (
            (StarSize::Ib, StarType::O, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::Ib, StarType::O, 5),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::Ib, StarType::B, 0),
            ZoneTable {
                inside: 0,
                hot: 7,
                inner: 12,
                habitable: 13,
                outer: 14
            }
        ),
        (
            (StarSize::Ib, StarType::B, 5),
            ZoneTable {
                inside: 0,
                hot: 5,
                inner: 10,
                habitable: 11,
                outer: 14
            }
        ),
        (
            (StarSize::Ib, StarType::A, 0),
            ZoneTable {
                inside: 0,
                hot: 4,
                inner: 10,
                habitable: 11,
                outer: 14
            }
        ),
        (
            (StarSize::Ib, StarType::A, 5),
            ZoneTable {
                inside: 0,
                hot: 4,
                inner: 9,
                habitable: 10,
                outer: 14
            }
        ),
        (
            (StarSize::Ib, StarType::F, 0),
            ZoneTable {
                inside: 0,
                hot: 4,
                inner: 9,
                habitable: 10,
                outer: 14
            }
        ),
        (
            (StarSize::Ib, StarType::F, 5),
            ZoneTable {
                inside: 0,
                hot: 3,
                inner: 9,
                habitable: 10,
                outer: 14
            }
        ),
        (
            (StarSize::Ib, StarType::G, 0),
            ZoneTable {
                inside: 0,
                hot: 3,
                inner: 9,
                habitable: 10,
                outer: 14
            }
        ),
        (
            (StarSize::Ib, StarType::G, 5),
            ZoneTable {
                inside: 1,
                hot: 4,
                inner: 9,
                habitable: 10,
                outer: 14
            }
        ),
        (
            (StarSize::Ib, StarType::K, 0),
            ZoneTable {
                inside: 2,
                hot: 4,
                inner: 9,
                habitable: 10,
                outer: 14
            }
        ),
        (
            (StarSize::Ib, StarType::K, 5),
            ZoneTable {
                inside: 2,
                hot: 4,
                inner: 9,
                habitable: 10,
                outer: 14
            }
        ),
        (
            (StarSize::Ib, StarType::M, 0),
            ZoneTable {
                inside: 3,
                hot: 4,
                inner: 9,
                habitable: 10,
                outer: 14
            }
        ),
        (
            (StarSize::Ib, StarType::M, 5),
            ZoneTable {
                inside: 3,
                hot: 4,
                inner: 9,
                habitable: 10,
                outer: 14
            }
        ),
        (
            (StarSize::II, StarType::O, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::II, StarType::O, 5),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::II, StarType::B, 0),
            ZoneTable {
                inside: 0,
                hot: 6,
                inner: 11,
                habitable: 12,
                outer: 13
            }
        ),
        (
            (StarSize::II, StarType::B, 5),
            ZoneTable {
                inside: 0,
                hot: 4,
                inner: 10,
                habitable: 11,
                outer: 13
            }
        ),
        (
            (StarSize::II, StarType::A, 0),
            ZoneTable {
                inside: 0,
                hot: 2,
                inner: 8,
                habitable: 9,
                outer: 13
            }
        ),
        (
            (StarSize::II, StarType::A, 5),
            ZoneTable {
                inside: 0,
                hot: 1,
                inner: 7,
                habitable: 8,
                outer: 13
            }
        ),
        (
            (StarSize::II, StarType::F, 0),
            ZoneTable {
                inside: 0,
                hot: 1,
                inner: 7,
                habitable: 8,
                outer: 13
            }
        ),
        (
            (StarSize::II, StarType::F, 5),
            ZoneTable {
                inside: 0,
                hot: 1,
                inner: 7,
                habitable: 8,
                outer: 13
            }
        ),
        (
            (StarSize::II, StarType::G, 0),
            ZoneTable {
                inside: 0,
                hot: 1,
                inner: 7,
                habitable: 8,
                outer: 13
            }
        ),
        (
            (StarSize::II, StarType::G, 5),
            ZoneTable {
                inside: 0,
                hot: 1,
                inner: 7,
                habitable: 8,
                outer: 13
            }
        ),
        (
            (StarSize::II, StarType::K, 0),
            ZoneTable {
                inside: 0,
                hot: 1,
                inner: 8,
                habitable: 9,
                outer: 13
            }
        ),
        (
            (StarSize::II, StarType::K, 5),
            ZoneTable {
                inside: 1,
                hot: 2,
                inner: 8,
                habitable: 9,
                outer: 13
            }
        ),
        (
            (StarSize::II, StarType::M, 0),
            ZoneTable {
                inside: 3,
                hot: 3,
                inner: 9,
                habitable: 10,
                outer: 13
            }
        ),
        (
            (StarSize::II, StarType::M, 5),
            ZoneTable {
                inside: 5,
                hot: 5,
                inner: 10,
                habitable: 11,
                outer: 13
            }
        ),
        (
            (StarSize::III, StarType::O, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::III, StarType::O, 5),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::III, StarType::B, 0),
            ZoneTable {
                inside: 0,
                hot: 6,
                inner: 11,
                habitable: 12,
                outer: 13
            }
        ),
        (
            (StarSize::III, StarType::B, 5),
            ZoneTable {
                inside: 0,
                hot: 4,
                inner: 9,
                habitable: 10,
                outer: 13
            }
        ),
        (
            (StarSize::III, StarType::A, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 7,
                habitable: 8,
                outer: 13
            }
        ),
        (
            (StarSize::III, StarType::A, 5),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 6,
                habitable: 7,
                outer: 13
            }
        ),
        (
            (StarSize::III, StarType::F, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 5,
                habitable: 6,
                outer: 13
            }
        ),
        (
            (StarSize::III, StarType::F, 5),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 5,
                habitable: 6,
                outer: 13
            }
        ),
        (
            (StarSize::III, StarType::G, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 5,
                habitable: 6,
                outer: 13
            }
        ),
        (
            (StarSize::III, StarType::G, 5),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 6,
                habitable: 7,
                outer: 13
            }
        ),
        (
            (StarSize::III, StarType::K, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 6,
                habitable: 7,
                outer: 13
            }
        ),
        (
            (StarSize::III, StarType::K, 5),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 7,
                habitable: 8,
                outer: 13
            }
        ),
        (
            (StarSize::III, StarType::M, 0),
            ZoneTable {
                inside: 0,
                hot: 1,
                inner: 7,
                habitable: 8,
                outer: 13
            }
        ),
        (
            (StarSize::III, StarType::M, 5),
            ZoneTable {
                inside: 3,
                hot: 3,
                inner: 8,
                habitable: 9,
                outer: 13
            }
        ),
        (
            (StarSize::IV, StarType::O, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::IV, StarType::O, 5),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::IV, StarType::B, 0),
            ZoneTable {
                inside: 0,
                hot: 6,
                inner: 11,
                habitable: 12,
                outer: 13
            }
        ),
        (
            (StarSize::IV, StarType::B, 5),
            ZoneTable {
                inside: 0,
                hot: 2,
                inner: 8,
                habitable: 9,
                outer: 13
            }
        ),
        (
            (StarSize::IV, StarType::A, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 6,
                habitable: 7,
                outer: 13
            }
        ),
        (
            (StarSize::IV, StarType::A, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 5,
                habitable: 6,
                outer: 13
            }
        ),
        (
            (StarSize::IV, StarType::F, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 4,
                habitable: 5,
                outer: 13
            }
        ),
        (
            (StarSize::IV, StarType::F, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 4,
                habitable: 5,
                outer: 13
            }
        ),
        (
            (StarSize::IV, StarType::G, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 4,
                habitable: 5,
                outer: 13
            }
        ),
        (
            (StarSize::IV, StarType::G, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 4,
                habitable: 5,
                outer: 13
            }
        ),
        (
            (StarSize::IV, StarType::K, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 3,
                habitable: 4,
                outer: 13
            }
        ),
        (
            (StarSize::IV, StarType::K, 5),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::IV, StarType::M, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::IV, StarType::M, 5),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::V, StarType::O, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::V, StarType::O, 5),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::V, StarType::B, 0),
            ZoneTable {
                inside: 0,
                hot: 5,
                inner: 11,
                habitable: 12,
                outer: 14
            }
        ),
        (
            (StarSize::V, StarType::B, 5),
            ZoneTable {
                inside: 0,
                hot: 2,
                inner: 8,
                habitable: 9,
                outer: 14
            }
        ),
        (
            (StarSize::V, StarType::A, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 6,
                habitable: 7,
                outer: 14
            }
        ),
        (
            (StarSize::V, StarType::A, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 5,
                habitable: 6,
                outer: 14
            }
        ),
        (
            (StarSize::V, StarType::F, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 4,
                habitable: 5,
                outer: 14
            }
        ),
        (
            (StarSize::V, StarType::F, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 3,
                habitable: 4,
                outer: 14
            }
        ),
        (
            (StarSize::V, StarType::G, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 2,
                habitable: 3,
                outer: 14
            }
        ),
        (
            (StarSize::V, StarType::G, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 2,
                habitable: 3,
                outer: 14
            }
        ),
        (
            (StarSize::V, StarType::K, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 1,
                habitable: 2,
                outer: 14
            }
        ),
        (
            (StarSize::V, StarType::K, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: -1,
                habitable: 0,
                outer: 14
            }
        ),
        (
            (StarSize::V, StarType::M, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: -1,
                habitable: 0,
                outer: 14
            }
        ),
        (
            (StarSize::V, StarType::M, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: -1,
                habitable: -1,
                outer: 14
            }
        ),
        (
            (StarSize::VI, StarType::O, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::VI, StarType::O, 5),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::VI, StarType::B, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::VI, StarType::B, 5),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::VI, StarType::A, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::VI, StarType::A, 5),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::VI, StarType::F, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::VI, StarType::F, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 2,
                habitable: 3,
                outer: 4
            }
        ),
        (
            (StarSize::VI, StarType::G, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 1,
                habitable: 2,
                outer: 4
            }
        ),
        (
            (StarSize::VI, StarType::G, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 0,
                habitable: 1,
                outer: 4
            }
        ),
        (
            (StarSize::VI, StarType::K, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 0,
                habitable: 0,
                outer: 4
            }
        ),
        (
            (StarSize::VI, StarType::K, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 0,
                habitable: 0,
                outer: 4
            }
        ),
        (
            (StarSize::VI, StarType::M, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 0,
                habitable: 0,
                outer: 4
            }
        ),
        (
            (StarSize::VI, StarType::M, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 0,
                habitable: 0,
                outer: 4
            }
        ),
        (
            (StarSize::D, StarType::O, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::D, StarType::O, 5),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::D, StarType::B, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: -1,
                habitable: 0,
                outer: 4
            }
        ),
        (
            (StarSize::D, StarType::B, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: -1,
                habitable: 0,
                outer: 4
            }
        ),
        (
            (StarSize::D, StarType::A, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: -1,
                habitable: -1,
                outer: 4
            }
        ),
        (
            (StarSize::D, StarType::A, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: -1,
                habitable: -1,
                outer: 4
            }
        ),
        (
            (StarSize::D, StarType::F, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: -1,
                habitable: -1,
                outer: 4
            }
        ),
        (
            (StarSize::D, StarType::F, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: -1,
                habitable: -1,
                outer: 4
            }
        ),
        (
            (StarSize::D, StarType::G, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: -1,
                habitable: -1,
                outer: 4
            }
        ),
        (
            (StarSize::D, StarType::G, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: -1,
                habitable: -1,
                outer: 4
            }
        ),
        (
            (StarSize::D, StarType::K, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: -1,
                habitable: -1,
                outer: 4
            }
        ),
        (
            (StarSize::D, StarType::K, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: -1,
                habitable: -1,
                outer: 4
            }
        ),
        (
            (StarSize::D, StarType::M, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: -1,
                habitable: -1,
                outer: 4
            }
        ),
        (
            (StarSize::D, StarType::M, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: -1,
                habitable: -1,
                outer: 4
            }
        ),
    ]);
}

lazy_static! {
    static ref LUMINOSITY_TABLE: HashMap<(StarType, u8, StarSize), f32> = HashMap::from_iter(vec![
        ((StarType::O, 0, StarSize::Ia), 0.0),
        ((StarType::O, 0, StarSize::Ib), 0.0),
        ((StarType::O, 0, StarSize::II), 0.0),
        ((StarType::O, 0, StarSize::III), 0.0),
        ((StarType::O, 0, StarSize::IV), 0.0),
        ((StarType::O, 0, StarSize::V), 0.0),
        ((StarType::O, 0, StarSize::VI), 0.0),
        ((StarType::O, 0, StarSize::D), 0.0),
        ((StarType::O, 5, StarSize::Ia), 0.0),
        ((StarType::O, 5, StarSize::Ib), 0.0),
        ((StarType::O, 5, StarSize::II), 0.0),
        ((StarType::O, 5, StarSize::III), 0.0),
        ((StarType::O, 5, StarSize::IV), 0.0),
        ((StarType::O, 5, StarSize::V), 0.0),
        ((StarType::O, 5, StarSize::VI), 0.0),
        ((StarType::O, 5, StarSize::D), 0.0),
        ((StarType::B, 0, StarSize::Ia), 560_000.0),
        ((StarType::B, 0, StarSize::Ib), 270_000.0),
        ((StarType::B, 0, StarSize::II), 170_000.0),
        ((StarType::B, 0, StarSize::III), 107_000.0),
        ((StarType::B, 0, StarSize::IV), 81_000.0),
        ((StarType::B, 0, StarSize::V), 56_000.0),
        ((StarType::B, 0, StarSize::VI), 0.0),
        ((StarType::B, 0, StarSize::D), 0.46),
        ((StarType::B, 5, StarSize::Ia), 204_000.0),
        ((StarType::B, 5, StarSize::Ib), 46_700.0),
        ((StarType::B, 5, StarSize::II), 18_600.0),
        ((StarType::B, 5, StarSize::III), 6_700.0),
        ((StarType::B, 5, StarSize::IV), 2_000.0),
        ((StarType::B, 5, StarSize::V), 1_400.0),
        ((StarType::B, 5, StarSize::VI), 0.0),
        ((StarType::B, 5, StarSize::D), 0.46),
        ((StarType::A, 0, StarSize::Ia), 107_000.0),
        ((StarType::A, 0, StarSize::Ib), 15_000.0),
        ((StarType::A, 0, StarSize::II), 2_200.0),
        ((StarType::A, 0, StarSize::III), 280.0),
        ((StarType::A, 0, StarSize::IV), 156.0),
        ((StarType::A, 0, StarSize::V), 90.0),
        ((StarType::A, 0, StarSize::VI), 0.0),
        ((StarType::A, 0, StarSize::D), 0.005),
        ((StarType::A, 5, StarSize::Ia), 81_000.0),
        ((StarType::A, 5, StarSize::Ib), 11_700.0),
        ((StarType::A, 5, StarSize::II), 850.0),
        ((StarType::A, 5, StarSize::III), 90.0),
        ((StarType::A, 5, StarSize::IV), 37.0),
        ((StarType::A, 5, StarSize::V), 16.0),
        ((StarType::A, 5, StarSize::VI), 0.0),
        ((StarType::A, 5, StarSize::D), 0.005),
        ((StarType::F, 0, StarSize::Ia), 61_000.0),
        ((StarType::F, 0, StarSize::Ib), 7_400.0),
        ((StarType::F, 0, StarSize::II), 600.0),
        ((StarType::F, 0, StarSize::III), 53.0),
        ((StarType::F, 0, StarSize::IV), 19.0),
        ((StarType::F, 0, StarSize::V), 8.1),
        ((StarType::F, 0, StarSize::VI), 0.0),
        ((StarType::F, 0, StarSize::D), 0.0003),
        ((StarType::F, 5, StarSize::Ia), 51_000.0),
        ((StarType::F, 5, StarSize::Ib), 5_100.0),
        ((StarType::F, 5, StarSize::II), 510.0),
        ((StarType::F, 5, StarSize::III), 43.0),
        ((StarType::F, 5, StarSize::IV), 12.0),
        ((StarType::F, 5, StarSize::V), 3.5),
        ((StarType::F, 5, StarSize::VI), 0.977),
        ((StarType::F, 5, StarSize::D), 0.0003),
        ((StarType::G, 0, StarSize::Ia), 67_000.0),
        ((StarType::G, 0, StarSize::Ib), 6_100.0),
        ((StarType::G, 0, StarSize::II), 560.0),
        ((StarType::G, 0, StarSize::III), 50.0),
        ((StarType::G, 0, StarSize::IV), 6.5),
        ((StarType::G, 0, StarSize::V), 1.21),
        ((StarType::G, 0, StarSize::VI), 0.322),
        ((StarType::G, 0, StarSize::D), 0.00006),
        ((StarType::G, 5, StarSize::Ia), 89_000.0),
        ((StarType::G, 5, StarSize::Ib), 8_100.0),
        ((StarType::G, 5, StarSize::II), 740.0),
        ((StarType::G, 5, StarSize::III), 75.0),
        ((StarType::G, 5, StarSize::IV), 4.9),
        ((StarType::G, 5, StarSize::V), 0.67),
        ((StarType::G, 5, StarSize::VI), 0.186),
        ((StarType::G, 5, StarSize::D), 0.00006),
        ((StarType::K, 0, StarSize::Ia), 100_000.0),
        ((StarType::K, 0, StarSize::Ib), 11_700.0),
        ((StarType::K, 0, StarSize::II), 890.0),
        ((StarType::K, 0, StarSize::III), 95.0),
        ((StarType::K, 0, StarSize::IV), 4.67),
        ((StarType::K, 0, StarSize::V), 0.42),
        ((StarType::K, 0, StarSize::VI), 0.117),
        ((StarType::K, 0, StarSize::D), 0.00004),
        ((StarType::K, 5, StarSize::Ia), 107_000.0),
        ((StarType::K, 5, StarSize::Ib), 20_400.0),
        ((StarType::K, 5, StarSize::II), 2_450.0),
        ((StarType::K, 5, StarSize::III), 320.0),
        ((StarType::K, 5, StarSize::IV), 0.0),
        ((StarType::K, 5, StarSize::V), 0.08),
        ((StarType::K, 5, StarSize::VI), 0.025),
        ((StarType::K, 5, StarSize::D), 0.00004),
        ((StarType::M, 0, StarSize::Ia), 117_000.0),
        ((StarType::M, 0, StarSize::Ib), 46_000.0),
        ((StarType::M, 0, StarSize::II), 4_600.0),
        ((StarType::M, 0, StarSize::III), 470.0),
        ((StarType::M, 0, StarSize::IV), 0.0),
        ((StarType::M, 0, StarSize::V), 0.04),
        ((StarType::M, 0, StarSize::VI), 0.011),
        ((StarType::M, 0, StarSize::D), 0.00003),
        ((StarType::M, 5, StarSize::Ia), 129_000.0),
        ((StarType::M, 5, StarSize::Ib), 89_000.0),
        ((StarType::M, 5, StarSize::II), 14_900.0),
        ((StarType::M, 5, StarSize::III), 2_280.0),
        ((StarType::M, 5, StarSize::IV), 0.0),
        ((StarType::M, 5, StarSize::V), 0.007),
        ((StarType::M, 5, StarSize::VI), 0.002),
        ((StarType::M, 5, StarSize::D), 0.00003),
    ]);
}

lazy_static! {
    static ref MASS_TABLE: HashMap<(StarType, u8, StarSize), f32> = HashMap::from_iter(vec![
        ((StarType::O, 0, StarSize::Ia), 0.0),
        ((StarType::O, 0, StarSize::Ib), 0.0),
        ((StarType::O, 0, StarSize::II), 0.0),
        ((StarType::O, 0, StarSize::III), 0.0),
        ((StarType::O, 0, StarSize::IV), 0.0),
        ((StarType::O, 0, StarSize::V), 0.0),
        ((StarType::O, 0, StarSize::VI), 0.0),
        ((StarType::O, 0, StarSize::D), 0.0),
        ((StarType::O, 5, StarSize::Ia), 0.0),
        ((StarType::O, 5, StarSize::Ib), 0.0),
        ((StarType::O, 5, StarSize::II), 0.0),
        ((StarType::O, 5, StarSize::III), 0.0),
        ((StarType::O, 5, StarSize::IV), 0.0),
        ((StarType::O, 5, StarSize::V), 0.0),
        ((StarType::O, 5, StarSize::VI), 0.0),
        ((StarType::O, 5, StarSize::D), 0.0),
        ((StarType::B, 0, StarSize::Ia), 60.0),
        ((StarType::B, 0, StarSize::Ib), 50.0),
        ((StarType::B, 0, StarSize::II), 30.0),
        ((StarType::B, 0, StarSize::III), 25.0),
        ((StarType::B, 0, StarSize::IV), 20.0),
        ((StarType::B, 0, StarSize::V), 18.0),
        ((StarType::B, 0, StarSize::VI), 0.0),
        ((StarType::B, 0, StarSize::D), 0.26),
        ((StarType::B, 5, StarSize::Ia), 30.0),
        ((StarType::B, 5, StarSize::Ib), 25.0),
        ((StarType::B, 5, StarSize::II), 20.0),
        ((StarType::B, 5, StarSize::III), 15.0),
        ((StarType::B, 5, StarSize::IV), 10.0),
        ((StarType::B, 5, StarSize::V), 6.5),
        ((StarType::B, 5, StarSize::VI), 0.0),
        ((StarType::B, 5, StarSize::D), 0.26),
        ((StarType::A, 0, StarSize::Ia), 18.0),
        ((StarType::A, 0, StarSize::Ib), 16.0),
        ((StarType::A, 0, StarSize::II), 14.0),
        ((StarType::A, 0, StarSize::III), 12.0),
        ((StarType::A, 0, StarSize::IV), 6.0),
        ((StarType::A, 0, StarSize::V), 3.2),
        ((StarType::A, 0, StarSize::VI), 0.0),
        ((StarType::A, 0, StarSize::D), 0.36),
        ((StarType::A, 5, StarSize::Ia), 15.0),
        ((StarType::A, 5, StarSize::Ib), 13.0),
        ((StarType::A, 5, StarSize::II), 11.0),
        ((StarType::A, 5, StarSize::III), 9.0),
        ((StarType::A, 5, StarSize::IV), 4.0),
        ((StarType::A, 5, StarSize::V), 2.1),
        ((StarType::A, 5, StarSize::VI), 0.0),
        ((StarType::A, 5, StarSize::D), 0.36),
        ((StarType::F, 0, StarSize::Ia), 13.0),
        ((StarType::F, 0, StarSize::Ib), 12.0),
        ((StarType::F, 0, StarSize::II), 10.0),
        ((StarType::F, 0, StarSize::III), 8.0),
        ((StarType::F, 0, StarSize::IV), 2.5),
        ((StarType::F, 0, StarSize::V), 1.7),
        ((StarType::F, 0, StarSize::VI), 0.0),
        ((StarType::F, 0, StarSize::D), 0.42),
        ((StarType::F, 5, StarSize::Ia), 12.0),
        ((StarType::F, 5, StarSize::Ib), 10.0),
        ((StarType::F, 5, StarSize::II), 8.1),
        ((StarType::F, 5, StarSize::III), 5.0),
        ((StarType::F, 5, StarSize::IV), 2.0),
        ((StarType::F, 5, StarSize::V), 1.3),
        ((StarType::F, 5, StarSize::VI), 0.8),
        ((StarType::F, 5, StarSize::D), 0.42),
        ((StarType::G, 0, StarSize::Ia), 12.0),
        ((StarType::G, 0, StarSize::Ib), 10.0),
        ((StarType::G, 0, StarSize::II), 8.1),
        ((StarType::G, 0, StarSize::III), 2.5),
        ((StarType::G, 0, StarSize::IV), 1.75),
        ((StarType::G, 0, StarSize::V), 1.04),
        ((StarType::G, 0, StarSize::VI), 0.6),
        ((StarType::G, 0, StarSize::D), 0.63),
        ((StarType::G, 5, StarSize::Ia), 13.0),
        ((StarType::G, 5, StarSize::Ib), 12.0),
        ((StarType::G, 5, StarSize::II), 10.0),
        ((StarType::G, 5, StarSize::III), 3.2),
        ((StarType::G, 5, StarSize::IV), 2.0),
        ((StarType::G, 5, StarSize::V), 0.94),
        ((StarType::G, 5, StarSize::VI), 0.528),
        ((StarType::G, 5, StarSize::D), 0.63),
        ((StarType::K, 0, StarSize::Ia), 14.0),
        ((StarType::K, 0, StarSize::Ib), 13.0),
        ((StarType::K, 0, StarSize::II), 11.0),
        ((StarType::K, 0, StarSize::III), 4.0),
        ((StarType::K, 0, StarSize::IV), 2.3),
        ((StarType::K, 0, StarSize::V), 0.825),
        ((StarType::K, 0, StarSize::VI), 0.43),
        ((StarType::K, 0, StarSize::D), 0.83),
        ((StarType::K, 5, StarSize::Ia), 18.0),
        ((StarType::K, 5, StarSize::Ib), 16.0),
        ((StarType::K, 5, StarSize::II), 14.0),
        ((StarType::K, 5, StarSize::III), 5.0),
        ((StarType::K, 5, StarSize::IV), 0.0),
        ((StarType::K, 5, StarSize::V), 0.57),
        ((StarType::K, 5, StarSize::VI), 0.33),
        ((StarType::K, 5, StarSize::D), 0.83),
        ((StarType::M, 0, StarSize::Ia), 20.0),
        ((StarType::M, 0, StarSize::Ib), 16.0),
        ((StarType::M, 0, StarSize::II), 14.0),
        ((StarType::M, 0, StarSize::III), 6.3),
        ((StarType::M, 0, StarSize::IV), 0.0),
        ((StarType::M, 0, StarSize::V), 0.489),
        ((StarType::M, 0, StarSize::VI), 0.154),
        ((StarType::M, 0, StarSize::D), 1.11),
        ((StarType::M, 5, StarSize::Ia), 25.0),
        ((StarType::M, 5, StarSize::Ib), 20.0),
        ((StarType::M, 5, StarSize::II), 16.0),
        ((StarType::M, 5, StarSize::III), 7.4),
        ((StarType::M, 5, StarSize::IV), 0.0),
        ((StarType::M, 5, StarSize::V), 0.331),
        ((StarType::M, 5, StarSize::VI), 0.104),
        ((StarType::M, 5, StarSize::D), 1.11),
    ]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::{Star, System};

    #[test_log::test]
    fn test_get_zone() {
        // Test case 1: Star Type B, Size Ia, Subtype 2
        let system1 = System {
            star: Star {
                star_type: StarType::B,
                size: StarSize::Ia,
                subtype: 2,
            },
            // Other fields can be left as default values for this test
            ..Default::default()
        };

        let zone1 = get_zone(&system1.star);
        debug!("zone1: {:?} for star {:?}", zone1, system1.star);
        assert_eq!(zone1.inside, 0);
        assert_eq!(zone1.hot, 7);
        assert_eq!(zone1.inner, 12);
        assert_eq!(zone1.habitable, 13);
        assert_eq!(zone1.outer, 14);

        // Test case 2: Star Type G, Size II, Subtype 8
        let system2 = System {
            star: Star {
                star_type: StarType::G,
                size: StarSize::II,
                subtype: 8,
            },
            ..Default::default()
        };

        let zone2 = get_zone(&system2.star);
        debug!("zone2: {:?} for star {:?}", zone2, system2.star);
        assert_eq!(zone2.inside, 0);
        assert_eq!(zone2.hot, 1);
        assert_eq!(zone2.inner, 7);
        assert_eq!(zone2.habitable, 8);
        assert_eq!(zone2.outer, 13);

        // Add more test cases as needed
    }
}
