# Callisto System Generation — booklet build

`source.md` is the rulebook exported from its Claude doc (the source of truth). `build.py` turns it into
`body.md` (tables rendered as LaTeX, footnote fixed, front matter stripped), runs pandoc to get `body.tex`,
and compiles `callisto.tex` with XeLaTeX three times (contents and list of tables need the passes).

- `callisto.tex` — memoir-class template: US Letter, two columns (multicol), Libertinus fonts from `fonts/`,
  the cover, contents, list of tables, running heads. The colour `cal` (#2B6CB0) is the rule, the wordmark and the headings.
- The Traveller wordmark uses Optima when the system has it (macOS does); otherwise Libertinus Sans.
- Tables are numbered by LaTeX in document order; `build.py` warns if the "Table N:" labels in the source are out of sequence.
- Wide tables (seven or more columns, or a lot of text) span both columns; the rest sit in the column they belong to.

Build: `./build.sh` (or `python3 build.py source.md`). Output: `callisto.pdf`.
