//! Names for Callisto's settled bodies.
//!
//! Book 6 draws from fixed lists (400 planet names, 338 moon names), and once
//! every settled world and every moon took a name, neighbouring systems kept
//! sharing them: nine systems in ten had a moon name in common with one within
//! two hexes. Here a name is built from a head, an optional link and an ending
//! in the lists' own style ("Thal" + "a" + "ssion"), tens of thousands of
//! them, so a repeat between neighbours is rare; within one system a name is
//! never used twice. Only bodies people live on are named: an empty moon is
//! designated from its planet instead ("Bulhai a"), as astronomers do.

use std::collections::HashSet;

use crate::util::rng_random_range;

const HEADS: &[&str] = &[
    "Al", "Am", "An", "Ar", "As", "Av", "Bal", "Bel", "Bor", "Cal", "Cas", "Cen", "Cor", "Cyr",
    "Dal", "Dar", "Del", "Dor", "El", "Em", "Er", "Es", "Fal", "Fen", "Fer", "Fis", "Gal", "Gar",
    "Hal", "Hel", "Hes", "Il", "Ir", "Is", "Jal", "Kal", "Kar", "Kel", "Kor", "Lan", "Lir", "Lor",
    "Lum", "Mal", "Mar", "Mer", "Mir", "Mor", "Nal", "Ner", "Nor", "Ol", "Or", "Os", "Pal", "Per",
    "Pyr", "Quel", "Ral", "Ren", "Rhal", "Ryl", "Sal", "Sar", "Sel", "Ser", "Sol", "Syl", "Tal",
    "Tar", "Tel", "Ter", "Thal", "Ther", "Tor", "Ul", "Ur", "Val", "Var", "Vel", "Ver", "Vor",
    "Xan", "Yl", "Zar", "Zel", "Zor", "Ast", "Brev", "Cael", "Drav", "Eld", "Hyr",
];

/// The vowel between head and ending; empty runs them together ("Thaldra").
const LINKS: &[&str] = &["", "a", "e", "i", "o", "u", "ae", "ia", "io"];

const ENDINGS: &[&str] = &[
    "dra", "dris", "gon", "lis", "lon", "mara", "mos", "nar", "nis", "nos", "phis", "ra", "ran",
    "ris", "ron", "ros", "sa", "sis", "ssion", "tha", "thos", "tis", "ton", "tria", "va", "van",
    "vis", "xis", "zar", "zon", "ria", "rin", "lia", "neth", "dor", "dun", "gar", "kis", "mir",
    "ndra", "rath", "stra", "thys", "vel", "wen",
];

/// One name, from the worldgen RNG (so a system's names come from its seed).
pub fn generate() -> String {
    let head = HEADS[rng_random_range(0..HEADS.len())];
    let link = LINKS[rng_random_range(0..LINKS.len())];
    let ending = ENDINGS[rng_random_range(0..ENDINGS.len())];
    format!("{head}{link}{ending}")
}

/// A name nothing in `taken` already has (compared without case), which is
/// then taken.
pub fn fresh(taken: &mut HashSet<String>) -> String {
    loop {
        let name = generate();
        if taken.insert(name.to_lowercase()) {
            return name;
        }
    }
}

/// The designation of a planet's `index`th unnamed moon, counting outward
/// from 0: `a`, `b`, … `z`, then `aa`, `ab`, ….
pub fn moon_letter(index: usize) -> String {
    let letter = |i: usize| char::from(b'a' + (i % 26) as u8);
    if index < 26 {
        letter(index).to_string()
    } else {
        format!("{}{}", letter(index / 26 - 1), letter(index))
    }
}

/// How many distinct names [`generate`] can make.
pub fn pool_size() -> usize {
    HEADS.len() * LINKS.len() * ENDINGS.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pool_is_large_and_its_names_are_distinct() {
        // Tens of thousands of combinations, nearly all of them different
        // strings (an empty link can make two spell the same).
        assert!(pool_size() > 35_000, "{}", pool_size());
        let all: HashSet<String> = HEADS
            .iter()
            .flat_map(|h| LINKS.iter().flat_map(move |l| ENDINGS.iter().map(move |e| format!("{h}{l}{e}"))))
            .collect();
        assert!(all.len() as f64 > pool_size() as f64 * 0.98, "{} distinct", all.len());
    }

    #[test]
    fn fresh_never_repeats() {
        let _rng = crate::util::RngScope::new(7);
        let mut taken = HashSet::new();
        let names: Vec<String> = (0..2_000).map(|_| fresh(&mut taken)).collect();
        let distinct: HashSet<&String> = names.iter().collect();
        assert_eq!(distinct.len(), names.len());
    }

    #[test]
    fn moon_letters() {
        assert_eq!(moon_letter(0), "a");
        assert_eq!(moon_letter(25), "z");
        assert_eq!(moon_letter(26), "aa");
        assert_eq!(moon_letter(27), "ab");
    }
}
