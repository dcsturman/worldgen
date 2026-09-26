#!/usr/bin/env python3
"""Build Callisto System Generation as a PDF.

Pipeline: source.md (exported from the rulebook doc) -> body.md (tables turned
into raw LaTeX, footnote fixed, front matter stripped) -> pandoc -> body.tex ->
xelatex with callisto.tex.

Usage: python3 build.py [source.md]
"""
import re, subprocess, sys, pathlib

SRC = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "source.md")
HERE = pathlib.Path(__file__).parent
WIDE_MIN_COLS = 6          # tables with this many columns or more span both columns
WIDE_TABLES = {5, 14, 20, 25}   # forced wide regardless of column count

def tex_escape(s: str) -> str:
    s = s.replace("\\", r"\textbackslash{}")
    for ch in "&%#$_{}":
        s = s.replace(ch, "\\" + ch)
    s = s.replace("~", r"\textasciitilde{}").replace("^", r"\textasciicircum{}")
    return s

def inline(md: str) -> str:
    """Minimal inline markdown -> LaTeX for table cells and captions."""
    out, i = [], 0
    # protect bold/italic markers before escaping
    tokens = re.split(r"(\*\*.+?\*\*|\*.+?\*|`.+?`)", md)
    for t in tokens:
        if t.startswith("**") and t.endswith("**"):
            out.append(r"\textbf{" + tex_escape(t[2:-2]) + "}")
        elif t.startswith("`") and t.endswith("`"):
            out.append(r"\texttt{" + tex_escape(t[1:-1]) + "}")
        elif t.startswith("*") and t.endswith("*") and len(t) > 2:
            out.append(r"\emph{" + tex_escape(t[1:-1]) + "}")
        else:
            out.append(tex_escape(t))
    s = "".join(out)
    # superscript figures that Libertinus has, keep; minus sign etc are fine in XeLaTeX
    return s

def split_row(line: str):
    line = line.strip()
    assert line.startswith("|") and line.endswith("|"), line
    return [c.strip() for c in line[1:-1].split("|")]

def render_table(num: int, caption: str, note: str, header, rows) -> str:
    ncols = len(header)
    # column classes: long text -> X (tabularx), else l ; blank spacer -> narrow
    maxlen = [max([len(header[c])] + [len(r[c]) if c < len(r) else 0 for r in rows]) for c in range(ncols)]
    wide = num in WIDE_TABLES or ncols >= WIDE_MIN_COLS or sum(maxlen) > 100
    spec = []
    any_x = False
    for c in range(ncols):
        if maxlen[c] == 0:
            spec.append("@{\\hspace{1.2em}}c@{}")
        elif maxlen[c] > 24:
            spec.append(">{\\RaggedRight\\arraybackslash}X"); any_x = True
        else:
            spec.append("l")
    width = r"\textwidth" if wide else r"\columnwidth"
    env = "tabularx" if any_x else "tabular"
    colspec = "".join(spec)
    lines = []
    if wide:
        lines.append(r"\end{multicols}")
        lines.append(r"\noindent\begin{minipage}{\textwidth}\centering\sffamily\footnotesize")
    else:
        lines.append(r"\begin{ruletable}\sffamily\footnotesize")
    lines.append(r"\captionof{table}[%s]{%s%s}" % (inline(caption), inline(caption),
                 (" \\normalfont\\small\\mdseries " + inline(note)) if note else ""))
    if env == "tabularx":
        lines.append(r"\begin{tabularx}{%s}{@{}%s@{}}" % (width, colspec))
    else:
        lines.append(r"\begin{tabular}{@{}%s@{}}" % colspec)
    lines.append(r"\toprule")
    lines.append(" & ".join(r"\textbf{" + inline(h) + "}" if h else "" for h in header) + r" \\")
    lines.append(r"\midrule")
    for r in rows:
        r = (r + [""] * ncols)[:ncols]
        # subheading rows (single bold cell, rest empty) get a little space above
        if r[0].startswith("**") and all(x == "" for x in r[1:]):
            lines.append(r"\addlinespace[0.4em]")
        lines.append(" & ".join(inline(c) for c in r) + r" \\")
    lines.append(r"\bottomrule")
    lines.append(r"\end{%s}" % env)
    if wide:
        lines.append(r"\end{minipage}\par\medskip")
        lines.append(r"\begin{multicols}{2}")
    else:
        lines.append(r"\end{ruletable}")
    return "```{=latex}\n" + "\n".join(lines) + "\n```\n"

def transform(md: str) -> str:
    lines = md.splitlines()
    # 1. strip title, subtitle and byline: keep from the first '## '
    start = next(i for i, l in enumerate(lines) if l.startswith("## "))
    lines = lines[start:]
    # 2. footnote: the paragraph beginning with the superscript one
    fn = None
    for i, l in enumerate(lines):
        if l.startswith("¹ "):
            fn = l[2:].strip()
            lines[i] = ""
            break
    if fn:
        fn = fn.strip("*").strip()
        lines = [l.replace("¹", "^[" + fn + "]", 1) if "same way.¹" in l else l for l in lines]
    # 3. tables
    out, i, seen = [], 0, []
    pending_caption = None
    while i < len(lines):
        l = lines[i]
        m = re.match(r"^\*\*Table (\d+): (.+?)\*\*\s*(.*)$", l)
        if m and i + 2 < len(lines) and lines[i + 1].strip() == "" and lines[i + 2].startswith("|"):
            num, cap, note = int(m.group(1)), m.group(2), m.group(3).strip()
            seen.append(num)
            j = i + 2
            tbl = []
            while j < len(lines) and lines[j].startswith("|"):
                tbl.append(lines[j]); j += 1
            header = split_row(tbl[0]); rows = [split_row(x) for x in tbl[2:]]
            out.append(render_table(num, cap, note, header, rows))
            i = j
            continue
        if l.startswith("|"):
            # a table without our caption line: render with an empty caption
            j = i; tbl = []
            while j < len(lines) and lines[j].startswith("|"):
                tbl.append(lines[j]); j += 1
            header = split_row(tbl[0]); rows = [split_row(x) for x in tbl[2:]]
            out.append(render_table(0, "", "", header, rows).replace(r"\captionof{table}[]{}", ""))
            i = j
            continue
        out.append(l)
        i += 1
    if seen != list(range(1, len(seen) + 1)):
        print("WARNING: table numbers out of sequence:", seen, file=sys.stderr)
    return "\n".join(out)

def main():
    md = SRC.read_text()
    body_md = transform(md)
    (HERE / "body.md").write_text(body_md)
    subprocess.run(["pandoc", "body.md", "-f", "markdown+raw_attribute", "-t", "latex",
                    "--top-level-division=section", "--wrap=none", "-o", "body.tex"], check=True, cwd=HERE)
    for _ in range(3):   # toc and list of tables need repeated runs
        r = subprocess.run(["xelatex", "-interaction=nonstopmode", "-halt-on-error", "callisto.tex"],
                           cwd=HERE, capture_output=True, text=True)
        if r.returncode != 0:
            log = (HERE / "callisto.log").read_text(errors="ignore")
            err = log[log.find("!"):][:3000]
            print(err); sys.exit(1)
    print("built callisto.pdf")

if __name__ == "__main__":
    main()
