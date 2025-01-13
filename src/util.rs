pub use rand::Rng;

pub(crate) fn arabic_to_roman(num: usize) -> String {
    if num > 20 {
        panic!("Input must be an integer between 0 and 20");
    }
    let roman_numerals: [(usize, &str); 21] = [
        (20, "XX"),
        (19, "XIX"),
        (18, "XVIII"),
        (17, "XVII"),
        (16, "XVI"),
        (15, "XV"),
        (14, "XIV"),
        (13, "XIII"),
        (12, "XII"),
        (11, "XI"),
        (10, "X"),
        (9, "IX"),
        (8, "VIII"),
        (7, "VII"),
        (6, "VI"),
        (5, "V"),
        (4, "IV"),
        (3, "III"),
        (2, "II"),
        (1, "I"),
        (0, "N"),
    ];
    for (value, symbol) in roman_numerals {
        if num >= value {
            return symbol.to_string();
        }
    }
    "".to_string()
}

// Functions
pub(crate) fn roll_2d6() -> i32 {
    let mut rng = rand::thread_rng();
    rng.gen_range(1..=6) + rng.gen_range(1..=6)
}

pub(crate) fn roll_1d6() -> i32 {
    let mut rng = rand::thread_rng();
    rng.gen_range(1..=6)
}

pub(crate) fn roll_10() -> i32 {
    let mut rng = rand::thread_rng();
    rng.gen_range(0..=9)
}
