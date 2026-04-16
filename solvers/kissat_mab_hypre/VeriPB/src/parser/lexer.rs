use crate::parser::error::ParseError;
use crate::parser::utils::{Position, Token};
use colored::Colorize;
use malachite_base::num::conversion::traits::FromStringBase;
use malachite_bigint::{BigInt, BigUint, Sign};
use malachite_nz::natural::Natural;
use memmap2::Mmap;
use std::iter::Peekable;
use std::mem;
use std::slice::Iter;

type Result<T> = std::result::Result<T, ParseError>;

#[derive(Debug, PartialEq, PartialOrd)]
pub enum IntegerParseMode {
    // Bits64 < Bits128 < BitsAny
    Bits64,
    Bits128,
    BitsAny,
}

fn is_letter_or_underscore(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

fn is_start_of_identifier_symbol(c: u8) -> bool {
    is_letter_or_underscore(c) || c == b'@' || c == b'$'
}

fn is_identifier_symbol(c: u8) -> bool {
    is_letter_or_underscore(c)
        || c.is_ascii_digit()
        || c == b'['
        || c == b']'
        || c == b'{'
        || c == b'}'
        || c == b'^'
        || c == b'-'
}

fn format_option(option: Option<&&u8>) -> String {
    if let Some(c) = option {
        format!("`{}`", char::from_u32(**c as u32).unwrap().escape_default())
    } else {
        "EOF".to_string()
    }
}

pub struct Lexer<'a> {
    mmap: Mmap,
    chars: Peekable<Iter<'a, u8>>,
    buffer: Vec<u8>,
    do_dynamic_integer_parse_mode: bool,
    integer_parse_mode: IntegerParseMode,
    prev_pos: Position,
    pos: Position,
    last_trace_pos: Position,
}

impl Lexer<'_> {
    pub fn new(mmap: Mmap) -> Self {
        // https://stackoverflow.com/questions/43952104/how-can-i-store-a-chars-iterator-in-the-same-struct-as-the-string-it-is-iteratin
        let chars = unsafe {
            mem::transmute::<
                std::iter::Peekable<std::slice::Iter<'_, u8>>,
                std::iter::Peekable<std::slice::Iter<'_, u8>>,
            >(mmap.iter().peekable())
        };
        Self {
            mmap,
            chars,
            buffer: Vec::with_capacity(1024),
            do_dynamic_integer_parse_mode: false,
            integer_parse_mode: IntegerParseMode::Bits64,
            prev_pos: Position {
                pos: 0,
                col: 1,
                line: 1,
            },
            pos: Position {
                pos: 0,
                col: 1,
                line: 1,
            },
            last_trace_pos: Position {
                pos: 0,
                col: 1,
                line: 1,
            },
        }
    }

    pub fn get_mmap(&self) -> &Mmap {
        &self.mmap
    }

    pub fn get_pos(&self) -> Position {
        self.prev_pos.clone()
    }

    pub fn get_line(&self) -> usize {
        self.prev_pos.line - 1
    }

    pub fn get_len(&self) -> usize {
        self.mmap.len()
    }

    pub fn set_integer_parse_mode(&mut self, mode: IntegerParseMode) {
        self.integer_parse_mode = mode;
    }

    pub fn enable_dynamic_integer_parse_mode(&mut self) {
        self.do_dynamic_integer_parse_mode = true;
    }

    pub fn disable_dynamic_integer_parse_mode(&mut self) {
        self.do_dynamic_integer_parse_mode = false;
        self.set_integer_parse_mode(IntegerParseMode::Bits64);
    }

    pub fn get_next_token(&mut self) -> Result<Token> {
        self.prev_pos = self.pos.clone();
        match self.chars.peek() {
            Some(b' ') => Ok(self.space()),
            Some(b'\t') => Ok(self.tab()),
            Some(b'\r') | Some(b'\n') => Ok(self.eol()),
            Some(b'%') => self.comment(),
            Some(c) if is_start_of_identifier_symbol(**c) => self.identifier(),
            Some(b'+') => self.plus_sign_or_integer(),
            Some(b'-') => self.minus_sign_or_maps_to_or_integer(),
            Some(c) if c.is_ascii_digit() => self.unsigned_integer(Sign::NoSign),
            Some(b'#') => self.proofgoal_id(),
            Some(b':') => Ok(self.single(Token::Colon)),
            Some(b';') => Ok(self.single(Token::Semicolon)),
            Some(b'~') => Ok(self.single(Token::Tilde)),
            Some(b'>') => self.greater_equal(),
            Some(b'<') => self.left_implication_or_less_equal(),
            Some(b'=') => self.equal_or_right_implication(),
            Some(b'*') => Ok(self.single(Token::Star)),
            Some(b'.') => Ok(self.single(Token::Dot)),
            Some(c) => Err(ParseError::new(
                format!("Unexpected character `{}`", std::ascii::escape_default(**c)),
                self.get_pos(),
                1,
                &self.mmap,
            )),
            None => Ok(Token::Eof),
        }
    }

    pub fn skip_to_eol(&mut self) -> Result<()> {
        self.comment()?;
        Ok(())
    }

    pub fn print_trace_since_last(&mut self) {
        let chunk = unsafe {
            std::str::from_utf8_unchecked(
                self.mmap
                    .get_unchecked(self.last_trace_pos.pos..self.pos.pos),
            )
        };
        let mut line_number = self.last_trace_pos.line;
        for line in chunk.lines() {
            let line = line.trim_ascii();
            if !line.is_empty() {
                println!(
                    "{}",
                    format!("line {line_number:>4}: {line}").bright_black()
                );
            };
            line_number += 1;
        }
        self.last_trace_pos = self.pos.clone();
    }

    fn space(&mut self) -> Token {
        let mut len = 1;
        self.chars.next();
        while let Some(b' ') = self.chars.peek() {
            len += 1;
            self.chars.next();
        }
        self.pos.pos += len;
        self.pos.col += len;
        Token::Space(len)
    }

    fn tab(&mut self) -> Token {
        let mut len = 1;
        self.chars.next();
        while let Some(b'\t') = self.chars.peek() {
            len += 1;
            self.chars.next();
        }
        self.pos.pos += len;
        self.pos.col += len;
        Token::Tab(len)
    }

    fn eol(&mut self) -> Token {
        let mut len = 0;
        if let Some(b'\r') = self.chars.peek() {
            self.chars.next();
            len += 1;
        };
        if let Some(b'\n') = self.chars.peek() {
            self.chars.next();
            len += 1;
        };
        self.pos.pos += len;
        self.pos.col = 1;
        self.pos.line += 1;
        Token::Eol(len)
    }

    fn comment(&mut self) -> Result<Token> {
        self.buffer.clear();
        self.chars.next();
        let mut len = 1;
        loop {
            len += 1;
            match self.chars.next() {
                Some(b'\n') => break,
                Some(b'\r') => {
                    if let Some(b'\n') = self.chars.peek() {
                        len += 1;
                        self.chars.next();
                    };
                    break;
                },
                None => return Err(ParseError::new(
                    "Expected newline (`\\r`, `\\n` or `\\r\\n`) at the end of comment but found EOF".to_string(),
                    self.get_pos(),
                    len,
                    &self.mmap,
                )),
                Some(c) => self.buffer.push(*c),
            };
        }
        self.pos.pos += len;
        self.pos.col = 1;
        self.pos.line += 1;
        Ok(Token::Comment(unsafe {
            String::from_utf8_unchecked(self.buffer.clone())
        }))
    }

    fn identifier(&mut self) -> Result<Token> {
        self.buffer.clear();
        self.buffer.push(*self.chars.next().unwrap());
        while let Some(c) = self.chars.peek() {
            if !is_identifier_symbol(**c) {
                break;
            };
            self.buffer.push(*self.chars.next().unwrap());
        }
        let len = self.buffer.len();
        if self.buffer[0] == b'@' && len == 1 {
            return Err(ParseError::new(
                "Expected complete label starting with `@` followed by at least one character but only found `@`".to_string(),
                self.get_pos(),
                len,
                &self.mmap,
            ));
        };
        if self.buffer[0] == b'$' && len == 1 {
            return Err(ParseError::new(
                "Expected auxiliary variable starting with `$` followed by at least one character but only found `$`".to_string(),
                self.get_pos(),
                len,
                &self.mmap,
            ));
        };
        self.pos.pos += len;
        self.pos.col += len;
        let identifier = unsafe { String::from_utf8_unchecked(self.buffer.clone()) };
        match self.buffer[0] {
            b'@' => Ok(Token::Label(identifier)),
            b'$' => Ok(Token::AuxiliaryVariable(identifier)),
            _ => Ok(Token::Identifier(identifier)),
        }
    }

    fn plus_sign_or_integer(&mut self) -> Result<Token> {
        self.chars.next();
        self.pos.pos += 1;
        self.pos.col += 1;
        match self.chars.peek() {
            Some(c) if c.is_ascii_digit() => self.unsigned_integer(Sign::Plus),
            _ => Ok(Token::Plus),
        }
    }

    fn minus_sign_or_maps_to_or_integer(&mut self) -> Result<Token> {
        self.chars.next();
        self.pos.pos += 1;
        self.pos.col += 1;
        match self.chars.peek() {
            Some(b'>') => {
                self.chars.next();
                self.pos.pos += 1;
                self.pos.col += 1;
                Ok(Token::MapsTo)
            }
            Some(c) if c.is_ascii_digit() => self.unsigned_integer(Sign::Minus),
            _ => Ok(Token::Minus),
        }
    }

    fn unsigned_integer(&mut self, sign: Sign) -> Result<Token> {
        self.buffer.clear();
        self.buffer.push(*self.chars.next().unwrap());
        while let Some(c) = self.chars.peek() {
            if !c.is_ascii_digit() {
                break;
            };
            self.buffer.push(**c);
            self.chars.next();
        }
        let result = unsafe { String::from_utf8_unchecked(self.buffer.clone()) };
        let len = self.buffer.len();
        let is_zero = self.buffer[0] == b'0';
        if is_zero && len > 1 {
            Err(ParseError::new(
                format!(
                    "The integer `{}{}` may not contain leading zeros",
                    if sign == Sign::Plus {
                        "+"
                    } else if sign == Sign::Minus && is_zero {
                        "-"
                    } else {
                        ""
                    },
                    result
                ),
                self.get_pos(),
                len,
                &self.mmap,
            ))
        } else {
            self.pos.pos += len;
            self.pos.col += len;
            let mode = match result {
                ref s if s.len() < 19 || (s.len() == 19 && &**s <= "9223372036854775807") => {
                    IntegerParseMode::Bits64
                }
                ref s
                    if s.len() < 39
                        || (s.len() == 39 && &**s <= "170141183460469231731687303715884105727") =>
                {
                    IntegerParseMode::Bits128
                }
                _ => IntegerParseMode::BitsAny,
            };
            if self.integer_parse_mode < mode {
                if self.do_dynamic_integer_parse_mode {
                    self.integer_parse_mode = mode;
                } else {
                    return Err(ParseError::new(
                        format!("The integer `{}{}` does not fit within 64 bits (as required in this context)", if sign == Sign::Plus {"+"} else if sign == Sign::Minus && is_zero {"-"} else {""}, result),
                        self.get_pos(),
                        len,
                        &self.mmap,
                    ));
                };
            };
            match self.integer_parse_mode {
                IntegerParseMode::Bits64 => {
                    let integer = if sign == Sign::Minus {
                        -result.parse::<i64>().unwrap()
                    } else {
                        result.parse::<i64>().unwrap()
                    };
                    Ok(Token::Integer64(sign, integer))
                }
                IntegerParseMode::Bits128 => {
                    let integer = if sign == Sign::Minus {
                        -result.parse::<i128>().unwrap()
                    } else {
                        result.parse::<i128>().unwrap()
                    };
                    Ok(Token::Integer128(sign, integer))
                }
                IntegerParseMode::BitsAny => {
                    // Hack to avoid checking UTF-8.
                    let natural = Natural::from_string_base(10, &result).unwrap();
                    let bigint = BigInt::from_biguint(
                        if sign == Sign::NoSign {
                            Sign::Plus
                        } else {
                            sign
                        },
                        Into::<BigUint>::into(natural),
                    );
                    Ok(Token::IntegerAny(sign, bigint))
                }
            }
        }
    }

    fn proofgoal_id(&mut self) -> Result<Token> {
        self.chars.next();
        self.pos.pos += 1;
        self.pos.col += 1;
        match self.chars.peek() {
            Some(c) if c.is_ascii_digit() => {
                match self.unsigned_integer(Sign::NoSign)? {
                    Token::Integer64(_, 0) => Err(ParseError::new(
                        "Expected a positive proofgoal ID but found `#0`".to_string(),
                        self.get_pos(),
                        2,
                        &self.mmap,
                    )),
                    Token::Integer64(_, id) => Ok(Token::ProofgoalID(id as usize)),
                    _ => panic!("UNREACHABLE"),
                }
            },
            option => Err(ParseError::new(
                format!("Expected complete proofgoal ID as `#` followed by an unsigned integer but found {} after `#`", format_option(option)),
                self.get_pos(),
                1,
                &self.mmap,
            )),
        }
    }

    fn greater_equal(&mut self) -> Result<Token> {
        self.chars.next();
        match self.chars.peek() {
            Some(b'=') => self.chars.next(),
            option => {
                return Err(ParseError::new(
                    format!(
                        "Expected complete operator `>=` but found {} as the second character",
                        format_option(option)
                    ),
                    self.get_pos(),
                    2,
                    &self.mmap,
                ))
            }
        };
        self.pos.pos += 2;
        self.pos.col += 2;
        Ok(Token::GreaterEqual)
    }

    fn left_implication_or_less_equal(&mut self) -> Result<Token> {
        self.chars.next();
        match self.chars.peek() {
            Some(b'=') => self.chars.next(),
            option => {
                return Err(ParseError::new(
                    format!(
                    "Expected complete operator `<=` or `<==` but found {} as the second character",
                    format_option(option)
                ),
                    self.get_pos(),
                    2,
                    &self.mmap,
                ))
            }
        };
        let (len, token) = match self.chars.peek() {
            Some(b'=') => {
                self.chars.next();
                (3, Token::LeftImplication)
            }
            _ => (2, Token::LessEqual),
        };
        self.pos.pos += len;
        self.pos.col += len;
        Ok(token)
    }

    fn equal_or_right_implication(&mut self) -> Result<Token> {
        self.chars.next();
        match self.chars.peek() {
            Some(b'=') => self.chars.next(),
            _ => {
                self.pos.pos += 1;
                self.pos.col += 1;
                return Ok(Token::Equal);
            }
        };
        match self.chars.peek() {
            Some(b'>') => self.chars.next(),
            option => {
                return Err(ParseError::new(
                    format!(
                        "Expected complete operator `==>` but found {} as the third character",
                        format_option(option)
                    ),
                    self.get_pos(),
                    3,
                    &self.mmap,
                ))
            }
        };
        self.pos.pos += 3;
        self.pos.col += 3;
        Ok(Token::RightImplication)
    }

    fn single(&mut self, token: Token) -> Token {
        self.chars.next();
        self.pos.pos += 1;
        self.pos.col += 1;
        token
    }
}
