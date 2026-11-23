//! docs-site: a small Rust site builder that turns `docs/` into a static
//! AssistX docs site with a design showcase.
//!
//! Subcommands: `check`, `build`, `serve`. Run from the `docs-site/` crate.

mod assets;
mod build;
mod check;
mod frontmatter;
mod markdown;
mod model;
mod roadmap;
mod serve;
mod showcase;
mod template;
mod theme;
mod util;

use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("help");

    // Derive paths from the crate manifest dir baked in at compile time.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest.parent().unwrap_or(&manifest).to_path_buf();
    let docs_dir = repo_root.join("docs");
    let out_dir = manifest.join("out");

    let exit = match cmd {
        "check" => run_check(&docs_dir, &repo_root),
        "build" => run_build(&docs_dir, &repo_root, &out_dir),
        "serve" => run_serve(&out_dir, &args[2..]),
        "help" | "-h" | "--help" => {
            print_help();
            0
        }
        other => {
            eprintln!("unknown command: {other}");
            print_help();
            2
        }
    };
    std::process::exit(exit);
}

fn run_check(docs_dir: &Path, repo_root: &Path) -> i32 {
    let issues = check::run(docs_dir, repo_root);
    if issues.is_empty() {
        println!("check: ok (no issues)");
        0
    } else {
        eprintln!("check: {} issue(s)", issues.len());
        for i in &issues {
            eprintln!("  {}: {}", i.file, i.problem);
        }
        1
    }
}

fn run_build(docs_dir: &Path, repo_root: &Path, out_dir: &Path) -> i32 {
    match build::run(docs_dir, repo_root, out_dir) {
        Ok(()) => {
            let idx = out_dir.join("index.html");
            if idx.exists() {
                println!("build: wrote {} ({} pages)", out_dir.display(), count_html(out_dir));
                0
            } else {
                eprintln!("build: finished but index.html is missing");
                1
            }
        }
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

fn run_serve(out_dir: &Path, rest: &[String]) -> i32 {
    if !out_dir.join("index.html").exists() {
        eprintln!("serve: nothing built yet; run `cargo run -j1 -- build` first");
        return 1;
    }
    let port = parse_port(rest).unwrap_or(3000);
    match serve::run(out_dir, port) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("serve: {e}");
            1
        }
    }
}

fn parse_port(rest: &[String]) -> Option<u16> {
    let mut iter = rest.iter();
    while let Some(a) = iter.next() {
        if a == "--port" {
            return iter.next().and_then(|v| v.parse().ok());
        }
        if let Ok(p) = a.parse::<u16>() {
            return Some(p);
        }
    }
    None
}

fn count_html(dir: &Path) -> usize {
    let mut n = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                n += count_html(&p);
            } else if p.extension().and_then(|x| x.to_str()) == Some("html") {
                n += 1;
            }
        }
    }
    n
}

fn print_help() {
    println!("docs-site -- a small Rust docs site builder");
    println!();
    println!("usage:");
    println!("  cargo run -j1 -- check          check docs/ (front matter, links, secrets)");
    println!("  cargo run -j1 -- build          render docs/ to docs-site/out/");
    println!("  cargo run -j1 -- serve [PORT]    serve docs-site/out/ on 127.0.0.1 (default 3000)");
}
