//! Helper functions for parser that can be reused for new parsers.

use std::{
    fs::File,
    io::{BufRead, BufReader, Lines},
    path::Path,
};

use crate::error::ParserError;

/// Get an iterator over the lines at the file with path `filename`.
#[inline]
pub fn get_lines<P>(filename: P) -> Result<Lines<BufReader<File>>, ParserError>
where
    P: AsRef<Path>,
{
    let file = File::open(&filename)
        .map_err(|_| ParserError::FileError(filename.as_ref().to_string_lossy().to_string()))?;
    Ok(BufReader::new(file).lines())
}

/// Get a buffered reader for the file at the path `filename`.
#[inline]
pub fn get_reader<P>(filename: P) -> Result<BufReader<File>, ParserError>
where
    P: AsRef<Path>,
{
    let file = File::open(&filename)
        .map_err(|_| ParserError::FileError(filename.as_ref().to_string_lossy().to_string()))?;
    Ok(BufReader::new(file))
}
