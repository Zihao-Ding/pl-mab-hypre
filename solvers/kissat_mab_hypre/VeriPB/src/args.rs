use std::path::PathBuf;

use clap::{ArgAction, Parser};

/// Command line arguments for the VeriPB tool.
#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Input formula file
    pub formula: PathBuf,

    /// Proof file
    pub derivation: PathBuf,

    /// Output formula file for problem reformulation
    pub output_formula: Option<PathBuf>,

    /// Force formula parsing to use the OPB parser
    #[arg(long, default_value_t, group = "formula_format")]
    pub opb: bool,

    /// Force formula parsing to use the DIMACS CNF parser
    #[arg(long, default_value_t, group = "formula_format")]
    pub cnf: bool,

    /// Force formula parsing to use the WCNF parser
    #[arg(long, default_value_t, group = "formula_format")]
    pub wcnf: bool,

    /// Elaborate proof to specified proof file
    #[arg(short, long, value_name = "OUTPUT_FILE", default_value = None)]
    pub elaborate: Option<PathBuf>,

    /// Enable trace mode
    #[arg(short, long, default_value_t = false, group = "progress")]
    pub trace: bool,

    /// Print the propagation trail for failed propagations
    #[arg(short = 'f', long)]
    pub trace_failed: bool,

    /// Disable checked deletion
    #[arg(short = 'u', long = "unchecked-deletion", action=ArgAction::SetFalse, default_value_t = true)]
    pub checked_deletion: bool,

    /// Fail proof instead of switching to unchecked deletion
    #[arg(short = 'c', long = "force-checked-deletion")]
    pub force_checked_deletion: bool,

    /// Disable printing the verification result
    #[arg(long = "disable-result-printing", action=ArgAction::SetFalse, default_value_t = true)]
    pub print_verification_result: bool,

    /// Disable printing of warnings
    #[arg(long = "hide-warnings", action=ArgAction::SetFalse, default_value_t = true)]
    pub show_warnings: bool,

    /// Disables the check that the canonical file path for the elaborated proof
    /// is different from the input files.
    #[arg(long = "ignore-file-path-check", default_value_t = false)]
    pub ignore_file_path_check: bool,

    /// Enables printing of the progress (only for proof version 3 and above).
    #[arg(short = 'p', long, default_value_t = false, group = "progress")]
    pub show_progress: bool,

    /// Enables collection and printing of checking statistics:
    /// Currently only supported statistic is total time (s)
    #[arg(short = 's', long, default_value_t = false)]
    pub stats: bool,
}

impl Default for Args {
    fn default() -> Self {
        Args {
            checked_deletion: true,
            force_checked_deletion: false,
            print_verification_result: true,
            formula: PathBuf::default(),
            derivation: PathBuf::default(),
            output_formula: None,
            elaborate: None,
            trace: false,
            opb: false,
            cnf: false,
            wcnf: false,
            trace_failed: false,
            show_warnings: true,
            show_progress: false,
            ignore_file_path_check: false,
            stats: false,
        }
    }
}
