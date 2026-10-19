// src/engine/lexer.rs

/// Defines the possible tokens that the lexer can produce.
/// Each token variant typically includes the lexeme's value (if applicable) and
/// its starting byte position in the source file for error reporting and LSP use.
#[derive(Debug, Clone)]
pub enum Token {
    /// A single left curly brace character: `{`. Stores its starting byte position.
    LBrace(usize),     // position
    /// A single right curly brace character: `}`. Stores its starting byte position.
    RBrace(usize),     // position
    /// A sequence of characters representing a variable name or keyword.
    /// Stores the identifier string and its starting byte position.
    Identifier(String, usize),
    /// A string literal enclosed in double quotes (e.g., `"hello world"`).
    /// Stores the string value (without quotes) and the starting byte position of the opening quote.
    StringLiteral(String, usize),
    /// An integer number (e.g., `123`). Stores the parsed 64-bit integer and its starting byte position.
    Number(i64, usize),
    /// A floating-point number (e.g., `1.23`). Stores the parsed 64-bit float and its starting byte position.
    Float(f64, usize),
    /// A boolean literal (`true` or `false`). Stores the boolean value and its starting byte position.
    Bool(bool, usize),
    /// A string whose source text is not its value: one written with an escape (`\"`, `\\`, `\n`,
    /// `\t`, `\r`, `\u{…}`) or as a raw string (`r"…"`, `r#"…"#`). Stores the value, the position of
    /// its first character (the `"` or the `r`), and the position just past its closing quote and fence.
    /// A string without either is still a `StringLiteral`, so a document that lexed before lexes to
    /// the same tokens.
    EscapedString(String, usize, usize),
    /// Text that cannot be read: an unknown escape, a string or block comment never closed. Stores
    /// what is wrong and the span it covers; the parser reports it as the document's error.
    Invalid(String, usize, usize),
}

/// The Lexer is responsible for consuming the source code string and turning it
/// into a sequence of meaningful tokens.
pub struct Lexer<'a> {
    /// A reference to the original source code string. Used primarily for string slicing
    /// if the token structure were to change, but here mainly for context.
    src: &'a str,
    /// A vector of characters derived from the source string. Using `Vec<char>` allows
    /// for O(1) indexing into Unicode code points, simplifying character-by-character
    /// lookups compared to iterating over a string slice directly.
    chars: Vec<char>,
    /// The current position (index) of the lexer within the `chars` vector.
    /// This represents the start of the next character to be processed.
    pos: usize,
}

impl<'a> Lexer<'a> {
    /// Creates a new Lexer instance for the given source code string.
    ///
    /// It consumes the entire source string into a `Vec<char>` on initialization.
    pub fn new(src: &'a str) -> Self {
        Self {
            src,
            chars: src.chars().collect(),
            pos: 0, // Start at the beginning of the file.
        }
    }

    /// Peeks at the next character in the stream without advancing the position.
    /// Returns `Some(char)` if a character exists, or `None` if the end of file (EOF) has been reached.
    fn peek(&self) -> Option<char> {
        // Use `.get()` to safely access the character at the current position.
        self.chars.get(self.pos).cloned()
    }

    /// Advances the lexer to the next character and returns the character that was just consumed.
    /// Returns `Some(char)` on success, or `None` if already at EOF.
    fn advance(&mut self) -> Option<char> {
        // Use `peek()` to get the current character.
        let ch = self.peek()?;
        // Increment the position to move past the consumed character.
        self.pos += 1;
        Some(ch)
    }

    /// Consumes and discards any contiguous sequence of whitespace characters.
    fn skip_whitespace(&mut self) {
        // Loop while the next character is present AND is a whitespace character.
        while matches!(self.peek(), Some(c) if c.is_whitespace()) {
            self.advance(); // Consume the whitespace character.
        }
    }

    /// The character `ahead` places past the current one, without advancing.
    fn peek_at(&self, ahead: usize) -> Option<char> {
        self.chars.get(self.pos + ahead).cloned()
    }

    /// Consumes whitespace and comments: the text no tool reads. `//` runs to the end of its line
    /// and `/* … */` to the first `*/` (they do not nest), the two forms the editor grammar and the
    /// language configuration have always declared. A block comment never closed comes back as an
    /// `Invalid` token, with the lexer at the end of the text.
    fn skip_trivia(&mut self) -> Option<Token> {
        loop {
            self.skip_whitespace();
            match (self.peek(), self.peek_at(1)) {
                (Some('/'), Some('/')) => {
                    while matches!(self.peek(), Some(c) if c != '\n') {
                        self.advance();
                    }
                }
                (Some('/'), Some('*')) => {
                    let start = self.pos;
                    self.pos += 2;
                    loop {
                        match (self.peek(), self.peek_at(1)) {
                            (Some('*'), Some('/')) => {
                                self.pos += 2;
                                break;
                            }
                            (Some(_), _) => {
                                self.advance();
                            }
                            (None, _) => {
                                return Some(Token::Invalid(
                                    "a block comment opened here is never closed with */".into(),
                                    start,
                                    start + 2,
                                ));
                            }
                        }
                    }
                }
                _ => return None,
            }
        }
    }

    /// The main tokenization function. Consumes the entire source code and returns
    /// a vector of all recognized tokens.
    pub fn tokenize(mut self) -> Vec<Token> {
        let mut tokens = Vec::new();

        // Loop until the end of the character stream is reached.
        while let Some(_ch) = self.peek() {
            // Always skip any leading whitespace and comments before trying to recognize a new token.
            if let Some(invalid) = self.skip_trivia() {
                tokens.push(invalid);
                break;
            }

            // Peek again after skipping to get the first non-whitespace character.
            let ch = match self.peek() {
                Some(c) => c,
                None => break, // If only whitespace remains, break the loop.
            };

            // Determine the type of token based on the current character.
            match ch {
                '{' => {
                    let start = self.pos; // Record the token's start position.
                    self.advance();      // Consume the '{' character.
                    tokens.push(Token::LBrace(start));
                }
                '}' => {
                    let start = self.pos; // Record the token's start position.
                    self.advance();      // Consume the '}' character.
                    tokens.push(Token::RBrace(start));
                }
                '"' => tokens.push(self.read_string()), // Start of a string literal.
                'r' if self.raw_string_ahead() => tokens.push(self.read_raw_string()), // r"…" or r#"…"#
                c if c.is_ascii_digit() => tokens.push(self.read_number()), // Start of a number literal.
                c if is_ident_start(c) => tokens.push(self.read_identifier_or_bool()), // Start of an identifier or keyword (like true/false).
                _ => {
                    // Handle unexpected characters by advancing and effectively skipping them.
                    // A proper lexer implementation would report an error here.
                    self.advance();
                }
            }
        }
        tokens
    }

    /// Reads a string literal token, starting from the opening double quote (`"`).
    ///
    /// A backslash starts an escape: `\"` a quote, `\\` a backslash, `\n` a newline, `\t` a tab,
    /// `\r` a carriage return, `\u{…}` the character with that hex code (1 to 6 digits), the same set
    /// task_history's diff writer emits. Any other escape is refused where it is written: before 0.2
    /// a backslash was an ordinary character, so a guessed meaning could silently change a value.
    /// A string may run over several lines, as it always could.
    fn read_string(&mut self) -> Token {
        let start = self.pos;
        self.advance(); // Consume the initial double quote (").

        let mut out = String::new();
        let mut escaped = false;
        let mut refused: Option<(String, usize, usize)> = None;
        loop {
            match self.peek() {
                None => {
                    return Token::Invalid(
                        "a string opened here is never closed with \"".into(),
                        start,
                        start + 1,
                    );
                }
                Some('"') => {
                    self.advance(); // Consume the final closing quote (").
                    break;
                }
                Some('\\') => {
                    escaped = true;
                    let at = self.pos;
                    self.advance();
                    match self.read_escape() {
                        Ok(ch) => out.push(ch),
                        Err(message) => {
                            refused.get_or_insert((message, at, self.pos));
                        }
                    }
                }
                Some(ch) => {
                    out.push(ch);
                    self.advance();
                }
            }
        }

        match refused {
            Some((message, from, to)) => Token::Invalid(message, from, to),
            // The position stored is the start of the opening quote.
            None if escaped => Token::EscapedString(out, start, self.pos),
            None => Token::StringLiteral(out, start),
        }
    }

    /// The character an escape stands for, the lexer just past its backslash. A refusal never
    /// consumes a quote or a line end, so the string's closing quote is still found.
    fn read_escape(&mut self) -> Result<char, String> {
        let unknown = |written: &str| {
            format!(
                "unknown escape \\{written} in a string; the escapes are \\\" \\\\ \\n \\t \\r \\u{{…}}, and a raw string r\"…\" holds a backslash as written"
            )
        };
        match self.peek() {
            Some('"') | Some('\\') | Some('n') | Some('t') | Some('r') => {
                let ch = self.advance().unwrap_or_default();
                Ok(match ch {
                    'n' => '\n',
                    't' => '\t',
                    'r' => '\r',
                    other => other,
                })
            }
            Some('u') => {
                self.advance();
                let malformed = "\\u is written \\u{…} with 1 to 6 hex digits".to_string();
                if self.peek() != Some('{') {
                    return Err(malformed);
                }
                self.advance();
                let mut hex = String::new();
                while let Some(c) = self.peek() {
                    if c == '}' || !c.is_ascii_hexdigit() || hex.len() == 6 {
                        break;
                    }
                    hex.push(c);
                    self.advance();
                }
                if hex.is_empty() || self.peek() != Some('}') {
                    return Err(malformed);
                }
                self.advance();
                u32::from_str_radix(&hex, 16)
                    .ok()
                    .and_then(char::from_u32)
                    .ok_or_else(|| format!("\\u{{{hex}}} is not a character"))
            }
            Some(other) if other != '\n' => {
                self.advance();
                Err(unknown(&other.to_string()))
            }
            _ => Err(unknown("")),
        }
    }

    /// Whether the `r` at the current position opens a raw string: `r`, any number of `#`, `"`.
    fn raw_string_ahead(&self) -> bool {
        let mut ahead = 1;
        while self.peek_at(ahead) == Some('#') {
            ahead += 1;
        }
        self.peek_at(ahead) == Some('"')
    }

    /// Reads a raw string, `r"…"` or `r#"…"#`: its text is its value, backslashes and all, and it
    /// ends at the first `"` followed by as many `#` as opened it.
    fn read_raw_string(&mut self) -> Token {
        let start = self.pos;
        self.advance(); // r
        let mut fence = 0;
        while self.peek() == Some('#') {
            fence += 1;
            self.advance();
        }
        self.advance(); // "
        let mut out = String::new();
        loop {
            match self.peek() {
                None => {
                    return Token::Invalid(
                        format!("a raw string opened here is never closed with \"{}", "#".repeat(fence)),
                        start,
                        start + fence + 2,
                    );
                }
                Some('"') if (1..=fence).all(|k| self.peek_at(k) == Some('#')) => {
                    self.pos += 1 + fence;
                    return Token::EscapedString(out, start, self.pos);
                }
                Some(ch) => {
                    out.push(ch);
                    self.advance();
                }
            }
        }
    }

    /// Reads a number literal token (either an integer or a floating-point number).
    ///
    /// It handles a single decimal point to identify a float.
    fn read_number(&mut self) -> Token {
        let start = self.pos;
        let mut out = String::new();
        let mut is_float = false;

        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                // Consume digits
                out.push(ch);
                self.advance();
            } else if ch == '.' && !is_float {
                // Consume the first decimal point, marking it as a float.
                is_float = true;
                out.push('.');
                self.advance();
            } else {
                // Stop reading on any other character (e.g., whitespace, operator, identifier).
                break;
            }
        }

        // After reading all parts of the number, parse the result.
        if is_float {
            // Panic on failed parse (e.g., "1.2.3" if this lexer allowed it, or large overflow)
            // A production lexer would return a Result<Token, Error>.
            Token::Float(out.parse::<f64>().unwrap(), start)
        } else {
            // Panic on failed parse.
            Token::Number(out.parse::<i64>().unwrap(), start)
        }
    }

    /// Reads an identifier or a recognized keyword (like `true` or `false`).
    fn read_identifier_or_bool(&mut self) -> Token {
        let start = self.pos;
        let mut out = String::new();

        // Read all alphanumeric characters and underscores.
        while let Some(ch) = self.peek() {
            if ch.is_alphanumeric() || ch == '_' {
                out.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        // Check if the read string matches a predefined keyword (Boolean literals).
        match out.as_str() {
            "true" => Token::Bool(true, start),
            "false" => Token::Bool(false, start),
            // If it's not a keyword, it's a generic identifier.
            _ => Token::Identifier(out, start),
        }
    }
}

/// Helper function to determine if a character is valid to start an identifier.
/// In this language, an identifier must start with an alphabet character or an underscore.
fn is_ident_start(ch: char) -> bool {
    ch.is_alphabetic() || ch == '_'
}





// // src/engine/lexer.rs
// 
// #[derive(Debug, Clone)]
// pub enum Token {
//     LBrace(usize),     // position
//     RBrace(usize),
//     Identifier(String, usize),
//     StringLiteral(String, usize),
//     Number(i64, usize),
//     Float(f64, usize),
//     Bool(bool, usize),
// }
// 
// pub struct Lexer<'a> {
//     src: &'a str,
//     chars: Vec<char>,
//     pos: usize,
// }
// 
// impl<'a> Lexer<'a> {
//     pub fn new(src: &'a str) -> Self {
//         Self {
//             src,
//             chars: src.chars().collect(),
//             pos: 0,
//         }
//     }
// 
//     fn peek(&self) -> Option<char> {
//         self.chars.get(self.pos).cloned()
//     }
// 
//     fn advance(&mut self) -> Option<char> {
//         let ch = self.peek()?;
//         self.pos += 1;
//         Some(ch)
//     }
// 
//     fn skip_whitespace(&mut self) {
//         while matches!(self.peek(), Some(c) if c.is_whitespace()) {
//             self.advance();
//         }
//     }
// 
//     pub fn tokenize(mut self) -> Vec<Token> {
//         let mut tokens = Vec::new();
// 
//         while let Some(ch) = self.peek() {
//             self.skip_whitespace();
// 
//             match ch {
//                 '{' => {
//                     let start = self.pos;
//                     self.advance();
//                     tokens.push(Token::LBrace(start));
//                 }
//                 '}' => {
//                     let start = self.pos;
//                     self.advance();
//                     tokens.push(Token::RBrace(start));
//                 }
//                 '"' => tokens.push(self.read_string()),
//                 c if c.is_ascii_digit() => tokens.push(self.read_number()),
//                 c if is_ident_start(c) => tokens.push(self.read_identifier_or_bool()),
//                 _ => {
//                     // Skip unexpected chars for now
//                     self.advance();
//                 }
//             }
//         }
//         tokens
//     }
// 
//     fn read_string(&mut self) -> Token {
//         let start = self.pos;
//         self.advance(); // consume "
// 
//         let mut out = String::new();
//         while let Some(ch) = self.peek() {
//             if ch == '"' {
//                 break;
//             }
//             out.push(ch);
//             self.advance();
//         }
//         self.advance(); // closing quote
//         Token::StringLiteral(out, start)
//     }
// 
//     fn read_number(&mut self) -> Token {
//         let start = self.pos;
//         let mut out = String::new();
//         let mut is_float = false;
// 
//         while let Some(ch) = self.peek() {
//             if ch.is_ascii_digit() {
//                 out.push(ch);
//                 self.advance();
//             } else if ch == '.' && !is_float {
//                 is_float = true;
//                 out.push('.');
//                 self.advance();
//             } else {
//                 break;
//             }
//         }
// 
//         if is_float {
//             Token::Float(out.parse::<f64>().unwrap(), start)
//         } else {
//             Token::Number(out.parse::<i64>().unwrap(), start)
//         }
//     }
// 
//     fn read_identifier_or_bool(&mut self) -> Token {
//         let start = self.pos;
//         let mut out = String::new();
// 
//         while let Some(ch) = self.peek() {
//             if ch.is_alphanumeric() || ch == '_' {
//                 out.push(ch);
//                 self.advance();
//             } else {
//                 break;
//             }
//         }
// 
//         match out.as_str() {
//             "true" => Token::Bool(true, start),
//             "false" => Token::Bool(false, start),
//             _ => Token::Identifier(out, start),
//         }
//     }
// }
// 
// fn is_ident_start(ch: char) -> bool {
//     ch.is_alphabetic() || ch == '_'
// }
// 