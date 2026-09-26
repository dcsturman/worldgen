#!/bin/sh
# Build Callisto System Generation. Requires pandoc and XeLaTeX (TeX Live) with memoir, multicol, booktabs, tabularx, caption, tikz, hyperref, microtype.
# 1. Export the rulebook doc as Markdown and save it as source.md next to this script.
# 2. Run: ./build.sh   ->  callisto.pdf
set -e
cd "$(dirname "$0")"
python3 build.py source.md
