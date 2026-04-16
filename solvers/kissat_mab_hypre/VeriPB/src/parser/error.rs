use crate::parser::utils::Position;
use memmap2::Mmap;

#[derive(Debug)]
pub struct ParseError {
    pub message: String,
    pub pos: Position,
    pub code: String,
}

impl std::error::Error for ParseError {}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{} at line {} col {}.\n{}",
            self.message, self.pos.line, self.pos.col, self.code
        )
    }
}

impl ParseError {
    pub fn new(message: String, pos: Position, len: usize, mmap: &Mmap) -> ParseError {
        let line = unsafe {
            std::str::from_utf8_unchecked(
                mmap.get_unchecked((pos.pos + 1 - pos.col)..(pos.pos + len)),
            )
            .replace('\t', "    ")
        };
        Self {
            message,
            pos,
            code: format!(
                "{}\n{}{}",
                line,
                " ".repeat(std::cmp::max(line.len(), 1) - len),
                "^".repeat(std::cmp::max(len, 1)),
            ),
        }
    }
}
