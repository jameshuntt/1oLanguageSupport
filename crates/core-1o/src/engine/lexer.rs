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

    /// The main tokenization function. Consumes the entire source code and returns
    /// a vector of all recognized tokens.
    pub fn tokenize(mut self) -> Vec<Token> {
        let mut tokens = Vec::new();

        // Loop until the end of the character stream is reached.
        while let Some(_ch) = self.peek() {
            // Always skip any leading whitespace before trying to recognize a new token.
            self.skip_whitespace();

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
    /// Note: This simple implementation does not handle escape sequences (e.g., `\"` or `\n`).
    fn read_string(&mut self) -> Token {
        let start = self.pos;
        self.advance(); // Consume the initial double quote (").

        let mut out = String::new();
        // Keep reading characters until the closing quote is found.
        while let Some(ch) = self.peek() {
            if ch == '"' {
                break; // Found the closing quote.
            }
            out.push(ch); // Append the character to the string value.
            self.advance(); // Consume the character.
        }
        self.advance(); // Consume the final closing quote (").

        // The position stored is the start of the opening quote.
        Token::StringLiteral(out, start)
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