//! The Callisto rulebook's tables, loaded from `docs/callisto/tables.json`.
//!
//! The JSON is exported from the rulebook itself: 41 tables, each with its
//! `number`, `title`, `note`, `header` and `rows`, every cell the printed
//! string. It is compiled in with `include_str!` (like `data/overrides.json`),
//! so the Dockerfile and `.dockerignore` both have to name it.
//!
//! Two layers:
//! - [`RawTable`] is the table as printed. Some tables are printed as two
//!   column groups side by side (Tables 11, 12, 14, 32, 33, 34, separated by
//!   an empty header cell); [`RawTable::unfolded_rows`] stacks them back into
//!   one list.
//! - [`DiceTable`] is a table whose first column is a die roll ("2D" or
//!   "1D"), with each row's roll text ("7 or less", "8 to 10", "12") parsed
//!   into an inclusive range, so a generator can look up a modified roll.

use std::ops::RangeInclusive;
use std::sync::LazyLock;

use serde::Deserialize;

const TABLES_JSON: &str = include_str!("../../docs/callisto/tables.json");

/// One rulebook table, as printed.
#[derive(Debug, Clone, Deserialize)]
pub struct RawTable {
    pub number: u32,
    pub title: String,
    pub note: String,
    pub header: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

static TABLES: LazyLock<Vec<RawTable>> = LazyLock::new(|| {
    serde_json::from_str(TABLES_JSON).expect("docs/callisto/tables.json is malformed")
});

/// Every table, in rulebook order.
pub fn all() -> &'static [RawTable] {
    &TABLES
}

/// The table printed as "Table `number`".
///
/// # Panics
/// If the rulebook has no such table: the generator names tables by number,
/// so a missing one is a mismatch between code and rulebook, not bad input.
pub fn table(number: u32) -> &'static RawTable {
    all()
        .iter()
        .find(|t| t.number == number)
        .unwrap_or_else(|| panic!("Callisto rulebook has no Table {number}"))
}

impl RawTable {
    /// The header and rows with side-by-side column groups stacked into one.
    ///
    /// A table printed in two halves has an empty header cell between them;
    /// the halves' rows are concatenated, left half first, and rows that are
    /// entirely blank (the padding under a shorter half) are dropped.
    pub fn unfolded_rows(&self) -> (Vec<&str>, Vec<Vec<&str>>) {
        let groups: Vec<RangeInclusive<usize>> = self
            .header
            .split(|h| h.is_empty())
            .scan(0, |start, group| {
                let range = *start..=*start + group.len() - 1;
                *start += group.len() + 1;
                Some(range)
            })
            .collect();
        let header = self.header[groups[0].clone()]
            .iter()
            .map(String::as_str)
            .collect();
        let rows = groups
            .iter()
            .flat_map(|g| {
                self.rows
                    .iter()
                    .map(move |row| row[g.clone()].iter().map(String::as_str).collect::<Vec<_>>())
            })
            .filter(|row| row.iter().any(|cell| !cell.is_empty()))
            .collect();
        (header, rows)
    }

    /// This table as a [`DiceTable`], if its first column is a die roll.
    pub fn dice(&self) -> Option<DiceTable> {
        let (header, rows) = self.unfolded_rows();
        let dice = match *header.first()? {
            "1D" => Dice::OneD,
            "2D" => Dice::TwoD,
            _ => return None,
        };
        let rows = rows
            .into_iter()
            .map(|row| {
                let range = parse_roll(row[0]).unwrap_or_else(|| {
                    panic!("Table {}: can't read roll {:?}", self.number, row[0])
                });
                (range, row[1..].iter().map(|c| c.to_string()).collect())
            })
            .collect();
        Some(DiceTable {
            number: self.number,
            dice,
            header: header[1..].iter().map(|h| h.to_string()).collect(),
            rows,
        })
    }
}

/// Which dice a table is rolled with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dice {
    OneD,
    TwoD,
}

impl Dice {
    /// The unmodified results the dice can show.
    pub fn range(self) -> RangeInclusive<i32> {
        match self {
            Dice::OneD => 1..=6,
            Dice::TwoD => 2..=12,
        }
    }
}

/// A table read by rolling dice: each row covers a range of results.
#[derive(Debug, Clone)]
pub struct DiceTable {
    pub number: u32,
    pub dice: Dice,
    /// Column headings after the roll column.
    pub header: Vec<String>,
    /// Each row's roll range and its cells after the roll column. Open-ended
    /// rows ("7 or less", "11 or more") extend to `i32::MIN` / `i32::MAX`.
    pub rows: Vec<(RangeInclusive<i32>, Vec<String>)>,
}

impl DiceTable {
    /// The row a (modified) roll lands on.
    ///
    /// A DM can push a roll past the printed rows; it then reads the nearest
    /// end of the table, which is how the rulebook's tables are read.
    pub fn lookup(&self, roll: i32) -> &[String] {
        let lo = self.rows.iter().map(|(r, _)| *r.start()).min().unwrap_or(roll);
        let hi = self.rows.iter().map(|(r, _)| *r.end()).max().unwrap_or(roll);
        let roll = roll.clamp(lo, hi);
        self.rows
            .iter()
            .find(|(r, _)| r.contains(&roll))
            .map(|(_, cells)| cells.as_slice())
            .unwrap_or_else(|| panic!("Table {}: no row for roll {roll}", self.number))
    }
}

/// Parse a printed roll cell into the range of results it covers.
///
/// The rulebook writes rolls as "7", "8 to 10", "2 or 3", "7 or less" and
/// "11 or more".
pub fn parse_roll(cell: &str) -> Option<RangeInclusive<i32>> {
    let cell = cell.trim();
    if let Some(n) = cell.strip_suffix(" or less") {
        return Some(i32::MIN..=n.trim().parse().ok()?);
    }
    if let Some(n) = cell.strip_suffix(" or more") {
        return Some(n.trim().parse().ok()?..=i32::MAX);
    }
    if let Some((a, b)) = cell.split_once(" to ").or_else(|| cell.split_once(" or ")) {
        return Some(a.trim().parse().ok()?..=b.trim().parse().ok()?);
    }
    cell.parse().ok().map(|n| n..=n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_table_is_present_in_order() {
        let numbers: Vec<u32> = all().iter().map(|t| t.number).collect();
        assert_eq!(numbers, (1..=41).collect::<Vec<_>>());
    }

    #[test]
    fn every_row_matches_its_header() {
        for t in all() {
            for row in &t.rows {
                assert_eq!(row.len(), t.header.len(), "Table {} row {row:?}", t.number);
            }
        }
    }

    #[test]
    fn roll_cells_parse() {
        assert_eq!(parse_roll("7"), Some(7..=7));
        assert_eq!(parse_roll("8 to 10"), Some(8..=10));
        assert_eq!(parse_roll("2 or 3"), Some(2..=3));
        assert_eq!(parse_roll("7 or less"), Some(i32::MIN..=7));
        assert_eq!(parse_roll("11 or more"), Some(11..=i32::MAX));
        assert_eq!(parse_roll("Rare"), None);
    }

    /// IMPLEMENTATION.md §8: each dice table covers its dice with no gaps,
    /// and (our addition) no overlaps, so every roll reads exactly one row.
    #[test]
    fn dice_tables_cover_their_dice_exactly_once() {
        let dice_tables: Vec<DiceTable> = all().iter().filter_map(RawTable::dice).collect();
        let numbers: Vec<u32> = dice_tables.iter().map(|t| t.number).collect();
        assert_eq!(
            numbers,
            [2, 3, 4, 7, 8, 11, 12, 15, 16, 17, 18, 19, 21, 29, 32, 33, 34, 37, 39],
            "the set of dice tables changed; check the new one is meant to be rolled"
        );
        for t in &dice_tables {
            for roll in t.dice.range() {
                let hits = t.rows.iter().filter(|(r, _)| r.contains(&roll)).count();
                assert_eq!(hits, 1, "Table {}: roll {roll} matches {hits} rows", t.number);
            }
        }
    }

    #[test]
    fn split_tables_unfold_in_order() {
        let t12 = table(12).dice().unwrap();
        let ratios: Vec<&str> = t12.rows.iter().map(|(_, c)| c[0].as_str()).collect();
        assert_eq!(
            ratios,
            ["1.25", "1.35", "1.45", "1.55", "1.65", "1.75", "1.90", "2.05", "2.25", "2.50", "2.80"]
        );
        let (header, rows) = table(14).unfolded_rows();
        assert_eq!(header, ["Position (HD)", "Zone", "Travel ×"]);
        assert_eq!(rows.first().unwrap()[0], "0.05");
        assert_eq!(rows.last().unwrap()[0], "100");
    }

    #[test]
    fn lookup_clamps_modified_rolls_to_the_table() {
        let t3 = table(3).dice().unwrap();
        assert_eq!(t3.lookup(8), ["G", "M"]);
        assert_eq!(t3.lookup(0), t3.lookup(2));
        assert_eq!(t3.lookup(14), t3.lookup(12));
        // Table 2 has open-ended rows: DM −1 on a 2 still reads "One".
        assert_eq!(table(2).dice().unwrap().lookup(1), ["One"]);
    }
}
