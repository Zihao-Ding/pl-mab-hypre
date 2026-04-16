#![allow(clippy::restriction)]

pub mod args;
pub mod context;
pub mod database;
pub mod deletion_sequence;
pub mod elaborator;
pub mod error;
pub mod misc_tokens;
pub mod occurrence_list;
pub mod order;
pub mod order_context;
pub mod parser;
pub mod prelude;
pub mod proofgoal;
pub mod rules;
pub mod subproof_context;
pub mod utils;
pub mod verifier;

use std::{
    ffi::OsStr,
    fs::{self, File},
    env,
    io::Write,
    time::Instant,
};

use ahash::AHashMap;
use anyhow::anyhow;
use args::Args;
use error::VeriPBError;
use memmap2::Mmap;
use parser::{error::ParseError, parser::Parser, utils::Position};
use prelude::*;
use veripb_formula::prelude::*;
use veripb_parser::{
    cnf_parser::parse_cnf_from_file, opb_parser::parse_opb_from_file,
    wcnf_parser::parse_wcnf_from_file,
};

/// Check if the elaborated proof path is different from the formula or derivation path.
///
/// This does not actually check if the elaborated proof file is the same as the formula or derivation file, but only compares the canonical paths of these files.
fn check_canonical_paths_are_different(args: &Args) -> anyhow::Result<()> {
    let elaborate_path = if let Some(elaborate) = &args.elaborate {
        match fs::canonicalize(elaborate) {
            Ok(path) => path,
            Err(_) => return Ok(()),
        }
    } else {
        return Ok(());
    };
    let formula_path = fs::canonicalize(&args.formula).map_err(|_| {
        anyhow!(
            "The formula file '{}' does not exist.",
            args.formula.display()
        )
    })?;
    let derivation_path = fs::canonicalize(&args.derivation).map_err(|_| {
        anyhow!(
            "The derivation file '{}' does not exist.",
            args.derivation.display()
        )
    })?;

    let bad_path = if formula_path == elaborate_path {
        "formula"
    } else if derivation_path == elaborate_path {
        "derivation"
    } else {
        return Ok(());
    };

    println!(
        "Warning: canonical path of {bad_path} is identical to canonical path of elaboration output. \n\
        Reading and writing to the same file can cause VeriPB to behave unpredictably. \n\n\
        Rerun with --ignore-file-path-check to proceed anyway."
    );

    Err(anyhow!(
        "Canonical paths are both: {}",
        elaborate_path.display()
    ))
}

/// Main library function to run the checker.
pub fn run_checker(args: Args) -> anyhow::Result<()> {
    // Auxiliary check about the formula, deriviation, and elaborated proof files.
    if !args.ignore_file_path_check {
        check_canonical_paths_are_different(&args)?;
    }

    let stats = args.stats;
    let time = if stats { Some(Instant::now()) } else { None };

    // Open deriviation file and create a memory map for it.
    let pbp_file = File::open(&args.derivation).map_err(|_| {
        anyhow!(
            "The derivation file '{}' does not exist.",
            args.derivation.display()
        )
    })?;

    let pbp_mmap = unsafe { Mmap::map(&pbp_file).unwrap() };

    // Parse the formula file. Try to automatically detect the correct parser depending on the file ending.
    let (formula, variables, formula_labels) = if args.cnf
        || (!args.opb && !args.wcnf && args.formula.extension() == Some(OsStr::new("cnf")))
    {
        let (formula, num_vars) = parse_cnf_from_file(&args.formula)?;
        let mut variables = VarNameManager::with_capacity(num_vars + 1);
        variables.add_by_name("");
        for i in 1..=num_vars {
            variables.add_by_name(&format!("x{i}"));
        }
        (formula, variables, AHashMap::new())
    } else if args.wcnf || (!args.opb && args.formula.extension() == Some(OsStr::new("wcnf"))) {
        let (formula, var_names) = parse_wcnf_from_file(&args.formula)?;
        (formula, var_names, AHashMap::new())
    } else {
        parse_opb_from_file(&args.formula)?
    };

    // Set up context and verifier.
    let context = Context::new(args, variables);
    let mut verifier = Verifier::new(context, formula)?;
    verifier.initialize()?;

    // Initialize parser and run the parser. The parser will dispatch the checking of each rule
    let mut parser = Parser::new(pbp_mmap, &mut verifier, formula_labels);
    match parser.parse() {
        Err(error) => {
            // Proof version is 2.0, so we use the 2.0 parse.
            if let VeriPBError::Parse(ParseError {
                pos:
                    Position {
                        pos: 0,
                        col: 1,
                        line: 0,
                    },
                ..
            }) = error
            {
                if verifier.context.args.print_verification_result {
                    println!("Info: Switched to proof version 2.0 (it is recommended to migrate to proof version 3.0).");
                }
                verifier.verify_file_version_2()?
            } else {
                Err(error)?
            }
        }
        result => result?,
    }

    // Warn the user that proof used unchecked assertions.
    if verifier.context.assumption_used && verifier.context.args.show_warnings {
        println!("Warning: The proof used unchecked assumptions.");
    }

    if stats {
        let total_time = time.unwrap().elapsed().as_secs_f64();
        println!("\nc statistic: time total: {total_time} s");
        // Other statistics to be added in future
    }

    if verifier.context.args.print_verification_result && verifier.context.has_conclusion {
        let args: Vec<String> = env::args().collect();
        let cnf_name = if args.len() > 1 {
            &args[1]
        } else {
            "unknown"
        };
        
        let verification_result = verifier.context.verification_result
            .as_deref()
            .unwrap_or("UNKNOWN");
        
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("verified.txt")
            .expect("无法打开结果文件");
        
        writeln!(file, "{}\t{:.6}\t{}", cnf_name, time.unwrap().elapsed().as_secs_f64(), verification_result)
            .expect("写入失败");
    }

    Ok(())
}
