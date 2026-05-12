use anyhow::Result;
use clap::Parser;
use git_version::git_version;
use mimalloc::MiMalloc;

use veripb::{args::Args, run_checker};

/// Use [mimalloc](https://github.com/microsoft/mimalloc) to improve allocation performance.
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

/// Main function of the VeriPB tool.
fn main() -> Result<()> {
    println!(
        "Running VeriPB version {}",
        git_version!(args = ["--tags"], fallback = env!("CARGO_PKG_VERSION"))
    );
    run_checker(Args::parse())
}
