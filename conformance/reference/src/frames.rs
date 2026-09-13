//! Bounded newline-delimited frame reader (STREAM sections 1 and 2).

use std::io::Read;

pub const PRE_NEGOTIATION_LIMIT: usize = 1_048_576;

pub enum Next {
    Frame(Vec<u8>),
    TooLarge,
    End,
}

pub struct FrameReader<R: Read> {
    input: R,
    buffer: Vec<u8>,
    pub limit: usize,
    eof: bool,
    /// Mutant `unbounded-frames`.
    pub unbounded: bool,
    /// Mutant `strict-off-by-one-limit`.
    pub off_by_one: bool,
    /// Mutant `parse-unterminated`.
    pub parse_unterminated: bool,
}

impl<R: Read> FrameReader<R> {
    pub fn new(input: R) -> Self {
        Self {
            input,
            buffer: Vec::new(),
            limit: PRE_NEGOTIATION_LIMIT,
            eof: false,
            unbounded: false,
            parse_unterminated: false,
            off_by_one: false,
        }
    }

    pub fn next(&mut self) -> std::io::Result<Next> {
        loop {
            if let Some(newline) = self.buffer.iter().position(|b| *b == b'\n') {
                if !self.unbounded
                    && (newline > self.limit || (self.off_by_one && newline == self.limit))
                {
                    return Ok(Next::TooLarge);
                }
                let frame: Vec<u8> = self.buffer.drain(..=newline).collect();
                return Ok(Next::Frame(frame[..frame.len() - 1].to_vec()));
            }
            if !self.unbounded && self.buffer.len() > self.limit {
                return Ok(Next::TooLarge);
            }
            if self.eof {
                if self.parse_unterminated && !self.buffer.is_empty() {
                    return Ok(Next::Frame(std::mem::take(&mut self.buffer)));
                }
                // An unterminated trailing frame is discarded (STREAM section 1.6).
                return Ok(Next::End);
            }
            let want = if self.unbounded {
                65_536
            } else {
                (self.limit + 1 - self.buffer.len()).clamp(1, 65_536)
            };
            let mut chunk = vec![0u8; want];
            let read = match self.input.read(&mut chunk) {
                Ok(read) => read,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            };
            if read == 0 {
                self.eof = true;
            } else {
                self.buffer.extend_from_slice(&chunk[..read]);
            }
        }
    }
}
