//! `orchestrator` — Phase 1 CLI.
//!
//! Points the parser at a `roadmap.yaml` (this repo's own, by default),
//! validates it, and prints the phase list plus the computed progress %.
//! This is the dogfood proof of Phase 1's exit criteria.
//!
//! Usage:
//!   orchestrator [path/to/roadmap.yaml]

use std::process::ExitCode;

use orchestrator::model::Status;
use orchestrator::parser;

fn main() -> ExitCode {
    let path = std::env::args().nth(1).unwrap_or_else(|| "roadmap.yaml".into());

    let roadmap = match parser::load(&path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    let p = &roadmap.project;
    println!("{} ({})  —  maturity: {:?}", p.name, p.repo, p.maturity);
    println!("{}", "─".repeat(56));

    for phase in &roadmap.phases {
        println!("  {} {:>2}. {}", marker(phase.status), phase.id, phase.name);
    }

    if !roadmap.parked.is_empty() {
        println!("  (parked, not counted:)");
        for item in &roadmap.parked {
            println!("    · {}", item.name);
        }
    }

    let prog = roadmap.progress();
    println!("{}", "─".repeat(56));
    println!(
        "progress: {}/{} phases done  ({:.0}%)",
        prog.done, prog.total, prog.percent
    );

    ExitCode::SUCCESS
}

/// A glanceable status marker for the terminal.
fn marker(status: Status) -> char {
    match status {
        Status::Done => '✓',
        Status::InProgress => '▸',
        Status::Blocked => '✗',
        Status::Todo => '·',
    }
}
