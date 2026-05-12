use std::{
    fs::File,
    io::{BufWriter, Error, Write},
    path::PathBuf,
};

use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum ElaborationError {
    #[error("Assumption cannot be elaborated.")]
    AssumptionUsed,

    #[error("The 'solx' does not exist in the kernel format yet.")]
    SolxNotInKernel,
}

#[derive(Debug)]
pub struct Elaborator {
    file: BufWriter<File>,
    use_buffer: bool,
    buffer: Vec<u8>,
    buffered_id: usize,
    pub current_id: usize,
    pub proof_buf: String,
}

const HEADER: &[u8] = b"pseudo-Boolean proof version 3.0\n";

impl Elaborator {
    /// Create a new output proof writing to the specified file.
    #[inline]
    pub fn new(path: PathBuf) -> Result<Self, Error> {
        let mut writer = BufWriter::new(File::create(path)?);
        writer.write_all(HEADER)?;
        Ok(Elaborator {
            file: writer,
            use_buffer: false,
            buffer: Vec::new(),
            buffered_id: 0,
            current_id: 0,
            proof_buf: String::new(),
        })
    }

    /// Write a string to the proof.
    #[inline]
    pub fn write(&mut self, buf: &str) {
        if self.use_buffer {
            self.buffer.extend(buf.as_bytes());
        } else {
            self.file.write_all(buf.as_bytes()).unwrap();
        }
    }

    /// Write a string to the proof and conclude the line with the newline symbol `\n`.
    #[inline]
    pub fn writeln(&mut self, buf: &str) {
        self.write(buf);
        self.end_line();
    }

    /// Write the newline symbol '\n' to the
    #[inline]
    pub fn end_line(&mut self) {
        self.write("\n");
    }

    /// Write loading the formula to the output proof.
    #[inline]
    pub fn load_formula(&mut self, formula_size: usize) {
        self.write("f ");
        self.write(&formula_size.to_string());
        self.writeln(";");
    }

    /// Increment the current ID and write the next ID to to the proof.
    #[inline]
    pub fn write_inc_id(&mut self) {
        self.current_id += 1;
        self.write(&self.current_id.to_string());
    }

    /// Increment the current ID that the `Elaborator` keeps track of and return the new incremented ID.
    #[inline]
    pub fn inc_id(&mut self) -> usize {
        self.current_id += 1;
        self.current_id
    }

    /// Decrement the current ID.
    #[inline]
    pub fn dec_id(&mut self) {
        self.current_id -= 1;
    }

    /// Reset proof string buffer.
    #[inline]
    pub fn reset_buf(&mut self) {
        self.proof_buf.clear();
    }

    /// Write the content of the proof buffer to the proof and clear it.
    #[inline]
    pub fn write_and_clear_buf(&mut self) {
        if self.use_buffer {
            self.buffer.extend(self.proof_buf.as_bytes());
        } else {
            self.file.write_all(self.proof_buf.as_bytes()).unwrap();
        }
        self.reset_buf();
    }

    /// Replace all occurrences of tilde in the `proof_buf` by
    #[inline]
    pub fn replace_tilde_write_and_clear_buf(&mut self, replace_to: &str) {
        self.proof_buf = self.proof_buf.replace("~", replace_to);
        self.write_and_clear_buf();
    }

    /// Write the proof to a buffer instead of the file until `write_buffered_proof` is called.
    ///
    /// The advantage of buffering is that no incorrect proof is ever written to the proof file, which might be a problem if the elaborated proof is streamed into the formally verified checker.
    #[inline]
    pub fn enable_buffered_proof(&mut self) {
        self.buffered_id = self.current_id;
        self.use_buffer = true;
    }

    /// Write the buffered proof to the file and reset the buffer.
    ///
    /// The advantage of buffering is that no incorrect proof is ever written to the proof file, which might be a problem if the elaborated proof is streamed into the formally verified checker.
    #[inline]
    pub fn write_buffered_proof(&mut self) {
        self.file.write_all(&self.buffer).unwrap();
        self.disable_buffered_proof();
    }

    /// Forget the buffered proof and reset the elaborator state to before the buffering started. This is especially important for the elaborated constraint ID.
    ///
    /// The advantage of buffering is that no incorrect proof is ever written to the proof file, which might be a problem if the elaborated proof is streamed into the formally verified checker.
    #[inline]
    pub fn forget_buffered_proof(&mut self) {
        self.current_id = self.buffered_id;
        self.disable_buffered_proof();
    }

    /// Disable buffer.
    ///
    /// The advantage of buffering is that no incorrect proof is ever written to the proof file, which might be a problem if the elaborated proof is streamed into the formally verified checker.
    #[inline]
    fn disable_buffered_proof(&mut self) {
        self.use_buffer = false;
        self.buffer.clear();
    }
}
