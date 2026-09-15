//! Everything the crate reads at compile time must reach the Docker build.
//!
//! `include_str!` and `include_bytes!` resolve relative to the source file, so
//! the data has to exist in the build stage or the crate simply won't compile.
//! Two separate things have to be true for that: `.dockerignore` must not
//! exclude it, and the Dockerfile — whose COPY steps are an explicit allowlist
//! — must name it.
//!
//! Getting one of the two right is the failure mode this guards. Adding
//! `data/overrides.json` with the ignore rule fixed but no COPY produced a
//! build that passed `cargo build`, passed `cargo test`, passed CI, passed a
//! wasm check, and then failed inside `docker build` with the compiler output
//! swallowed by BuildKit. The only signal was a deploy that didn't happen.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Every path `include_str!`/`include_bytes!` pulls in, relative to the repo
/// root.
fn included_paths(root: &Path) -> BTreeSet<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    let mut files = Vec::new();
    walk(&root.join("src"), &mut files);

    let mut found = BTreeSet::new();
    for f in files {
        let Ok(text) = std::fs::read_to_string(&f) else {
            continue;
        };
        for macro_name in ["include_str!", "include_bytes!"] {
            let mut rest = text.as_str();
            while let Some(i) = rest.find(macro_name) {
                rest = &rest[i + macro_name.len()..];
                let Some(start) = rest.find('"') else { break };
                let Some(len) = rest[start + 1..].find('"') else {
                    break;
                };
                let rel = &rest[start + 1..start + 1 + len];
                // Resolve relative to the including file's directory, then
                // make it repo-relative again.
                let mut p = f.parent().unwrap().to_path_buf();
                for part in rel.split('/') {
                    match part {
                        "." => {}
                        ".." => {
                            p.pop();
                        }
                        other => p.push(other),
                    }
                }
                if let Ok(stripped) = p.strip_prefix(root) {
                    found.insert(stripped.to_path_buf());
                }
                rest = &rest[start + 1 + len..];
            }
        }
    }
    found
}

#[test]
fn every_compile_time_include_reaches_the_docker_build() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dockerfile = std::fs::read_to_string(root.join("Dockerfile")).expect("Dockerfile exists");
    let dockerignore =
        std::fs::read_to_string(root.join(".dockerignore")).expect(".dockerignore exists");

    // The stages that compile Rust. Both need every included file.
    let copied: Vec<&str> = dockerfile
        .lines()
        .filter_map(|l| l.trim().strip_prefix("COPY "))
        .filter(|l| !l.starts_with("--from"))
        .flat_map(|l| l.split_whitespace())
        .collect();

    let reincluded: Vec<&str> = dockerignore
        .lines()
        .filter_map(|l| l.trim().strip_prefix('!'))
        .collect();

    for path in included_paths(root) {
        let s = path.to_string_lossy().replace('\\', "/");
        let top = s.split('/').next().unwrap().to_string();

        assert!(
            copied.iter().any(|c| {
                let c = c.trim_end_matches('/');
                c == s || c == top || c.ends_with(&format!("/{top}"))
            }),
            "{s} is pulled in at compile time but no Dockerfile COPY brings it \
             into the build. The COPY steps are an allowlist — add `COPY {top} \
             ./{top}/` to every stage that compiles Rust, or the image build \
             fails where cargo and CI both pass."
        );

        // `*` excludes everything, so it also has to be re-included.
        assert!(
            reincluded
                .iter()
                .any(|r| r.trim_end_matches('/') == top || r.trim_end_matches('/') == s),
            "{s} is pulled in at compile time but .dockerignore excludes it; \
             add `!{top}`."
        );
    }
}
