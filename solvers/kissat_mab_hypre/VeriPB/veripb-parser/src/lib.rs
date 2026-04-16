#![allow(clippy::restriction)]

//! Crate for parsing strings and/or files to VeriPB data structures.
//!
//! For example use of the parsing functions, have a look at `examples`. These can be called using
//! ```bash
//! cargo run -r --example <example_name>
//! ```
//!
//! # Supported Parsers
//! Parsers for the following functionality are implemented in this library:
//! - VeriPB assignment (see [`assignment_parser`])
//! - CNF parser (see [`cnf_parser`])
//! - OPB parser (see [`opb_parser`])
//! - VeriPB substitution (see [`substitution_parser`])
//! - WCNF parser (see [`wcnf_parser`])

pub mod assignment_parser;
pub mod assignment_token;
pub mod cnf_parser;
pub mod cnf_token;
pub mod error;
pub mod opb_parser;
pub mod opb_token;
pub mod parser;
pub mod substitution_parser;
pub mod substitution_token;
pub mod wcnf_parser;
pub mod wcnf_token;
