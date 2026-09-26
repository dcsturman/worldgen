//! Temperature: rulebook Section 7.5, Tables 24 to 27.
//!
//! Read, not rolled: an albedo class (Table 24), the equilibrium temperature
//! for the world's position and class (Table 25, which is a formula
//! tabulated), greenhouse warming by atmosphere (Table 26), and a band (Table
//! 27). The result is a global mean over the whole surface and year.

use serde::{Deserialize, Serialize};

use crate::callisto::tables;

/// Table 24's albedo classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlbedoClass {
    Rock,
    ThinAir,
    EarthLike,
    Cloudy,
    Ice,
}

impl AlbedoClass {
    /// The class's albedo, read from Table 24's "Column" cell.
    pub fn albedo(self) -> f32 {
        let row = match self {
            AlbedoClass::Rock => 0,
            AlbedoClass::ThinAir => 1,
            AlbedoClass::EarthLike => 2,
            AlbedoClass::Cloudy => 3,
            AlbedoClass::Ice => 4,
        };
        tables::table(24).rows[row][1]
            .parse()
            .expect("Table 24 albedos are numbers")
    }

    /// Which class a world is in (Table 24). Combinations the table doesn't
    /// name are read as the nearest it does: an airless world with water has
    /// ice (Section 13.2), and an exotic or unusual atmosphere over a dry
    /// surface is thin air.
    pub fn of(atmosphere: i32, hydro: i32, hydro_is_ice: bool) -> AlbedoClass {
        if hydro_is_ice {
            return AlbedoClass::Ice;
        }
        match (atmosphere, hydro) {
            (11 | 12, _) => AlbedoClass::Cloudy,
            (4.., 9..) => AlbedoClass::Cloudy,
            (4.., 4..=8) => AlbedoClass::EarthLike,
            (0..=1, 4..) => AlbedoClass::Ice,
            (2..=3, 4..) => AlbedoClass::ThinAir,
            (0..=3, _) => AlbedoClass::Rock,
            _ => AlbedoClass::ThinAir,
        }
    }
}

/// Table 27's bands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TempBand {
    Frozen,
    Cold,
    Temperate,
    Hot,
    Roasting,
}

impl TempBand {
    /// The band a mean temperature falls in, in whole degrees as Table 27
    /// prints them: Frozen −51 and below, Cold to 0, Temperate to 30, Hot to
    /// 80, Roasting above.
    pub fn of(celsius: f32) -> TempBand {
        match celsius.round() as i32 {
            i32::MIN..=-51 => TempBand::Frozen,
            -50..=0 => TempBand::Cold,
            1..=30 => TempBand::Temperate,
            31..=80 => TempBand::Hot,
            _ => TempBand::Roasting,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            TempBand::Frozen => "Frozen",
            TempBand::Cold => "Cold",
            TempBand::Temperate => "Temperate",
            TempBand::Hot => "Hot",
            TempBand::Roasting => "Roasting",
        }
    }
}

/// Equilibrium temperature in °C before greenhouse warming (Table 25's
/// formula): 278.5 K × (1 − A)^¼ ÷ √position.
pub fn equilibrium_c(position_hd: f32, albedo: f32) -> f32 {
    278.5 * (1.0 - albedo).powf(0.25) / position_hd.sqrt() - 273.15
}

/// Greenhouse warming for an atmosphere code (Table 26).
pub fn greenhouse_c(atmosphere: i32) -> f32 {
    let t = tables::table(26);
    let code = crate::util::value_to_ehex(atmosphere.clamp(0, 15) as u32);
    let col = t
        .header
        .iter()
        .position(|h| match h.split_once('–') {
            Some((a, b)) => {
                let (a, b) = (a.chars().next(), b.chars().next());
                a.zip(b).is_some_and(|(a, b)| (a..=b).contains(&code))
            }
            None => h.len() == 1 && h.starts_with(code),
        })
        .expect("Table 26 covers every atmosphere code");
    t.rows[0][col].parse().expect("Table 26 warmings are numbers")
}

/// A world's temperature: the reading, and what the checks made of it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Temperature {
    /// Mean surface temperature, °C.
    pub celsius: f32,
    pub band: TempBand,
    /// The albedo class the final reading used.
    pub albedo_class: AlbedoClass,
    /// The oceans have boiled: the hydrographics is in the air as steam.
    pub steam: bool,
}

/// Work out a world's temperature (Section 7.5), with the ice check: a Cold
/// or Frozen world with water freezes over and is read again in the Ice
/// column, once. A Roasting world with water has boiled its oceans: it is
/// read in the Cloudy column and stays Roasting whatever that says.
pub fn temperature(position_hd: f32, atmosphere: i32, hydro: i32, hydro_is_ice: bool) -> Temperature {
    let greenhouse = greenhouse_c(atmosphere);
    let read = |class: AlbedoClass| equilibrium_c(position_hd, class.albedo()) + greenhouse;
    let class = AlbedoClass::of(atmosphere, hydro, hydro_is_ice);
    let celsius = read(class);
    let band = TempBand::of(celsius);
    if hydro >= 1 && band <= TempBand::Cold && class != AlbedoClass::Ice {
        let celsius = read(AlbedoClass::Ice);
        return Temperature {
            celsius,
            band: TempBand::of(celsius),
            albedo_class: AlbedoClass::Ice,
            steam: false,
        };
    }
    if hydro >= 1 && band == TempBand::Roasting {
        return Temperature {
            celsius: read(AlbedoClass::Cloudy),
            band: TempBand::Roasting,
            albedo_class: AlbedoClass::Cloudy,
            steam: true,
        };
    }
    Temperature {
        celsius,
        band,
        albedo_class: class,
        steam: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// IMPLEMENTATION.md §8: Table 25 is its formula, in whole degrees.
    #[test]
    fn table_25_is_its_formula() {
        let classes = [
            AlbedoClass::Rock,
            AlbedoClass::ThinAir,
            AlbedoClass::EarthLike,
            AlbedoClass::Cloudy,
            AlbedoClass::Ice,
        ];
        let mut bad = Vec::new();
        for row in &tables::table(25).rows {
            let pos: f32 = row[0].parse().unwrap();
            for (i, c) in classes.iter().enumerate() {
                let printed: f32 = row[i + 1].replace('−', "-").parse().unwrap();
                let formula = equilibrium_c(pos, c.albedo());
                if (printed - formula).abs() > 1.0 {
                    bad.push(format!("{pos} {c:?}: printed {printed}, formula {formula:.1}"));
                }
            }
        }
        assert!(bad.is_empty(), "{bad:#?}");
    }

    #[test]
    fn greenhouse_reads_table_26() {
        let got: Vec<f32> = (0..=15).map(greenhouse_c).collect();
        assert_eq!(
            got,
            [0.0, 0.0, 5.0, 5.0, 20.0, 20.0, 35.0, 35.0, 60.0, 60.0, 50.0, 150.0, 250.0, 100.0, 10.0, 35.0]
        );
    }

    #[test]
    fn table_27_bands() {
        let printed: Vec<&str> = tables::table(27).rows.iter().map(|r| r[1].as_str()).collect();
        assert_eq!(
            printed,
            ["−51 °C and below", "−50 to 0 °C", "1 to 30 °C", "31 to 80 °C", "above 80 °C"]
        );
        assert_eq!(TempBand::of(-51.0), TempBand::Frozen);
        assert_eq!(TempBand::of(-50.0), TempBand::Cold);
        assert_eq!(TempBand::of(1.0), TempBand::Temperate);
        assert_eq!(TempBand::of(80.0), TempBand::Hot);
        assert_eq!(TempBand::of(81.0), TempBand::Roasting);
    }

    /// The rulebook's examples (Section 7.5).
    #[test]
    fn earth_mars_and_the_dense_world() {
        // Earth: 1.0, atmosphere 6, hydrographics 7: 17 °C, Temperate.
        let earth = temperature(1.0, 6, 7, false);
        assert_eq!((earth.celsius.round(), earth.band), (17.0, TempBand::Temperate));
        // Mars at 1.5: Rock, Frozen.
        assert_eq!(temperature(1.5, 1, 0, false).band, TempBand::Frozen);
        // 1.4, atmosphere 8, hydrographics 7: 2 °C, just Temperate.
        let dense = temperature(1.4, 8, 7, false);
        assert_eq!((dense.celsius.round(), dense.band), (2.0, TempBand::Temperate));
        // With hydrographics 9 it is Cloudy, Cold, and the ice check reads it
        // again at −32. The example calls that Frozen, but Table 27 puts −32
        // in Cold (−50 to 0), and the table wins.
        let cloudy = temperature(1.4, 8, 9, false);
        assert_eq!((cloudy.celsius.round(), cloudy.band), (-32.0, TempBand::Cold));
        assert_eq!(cloudy.albedo_class, AlbedoClass::Ice);
    }

    /// Noricum's steam world (12.3): 0.52, atmosphere 7, hydrographics 6.
    #[test]
    fn a_roasting_world_boils_its_oceans() {
        let t = temperature(0.52, 7, 6, false);
        assert!(t.steam);
        assert_eq!(t.band, TempBand::Roasting);
        assert_eq!(t.albedo_class, AlbedoClass::Cloudy);
    }
}
