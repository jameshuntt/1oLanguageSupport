// qweres/infra/language-server-1o/src/engine/parser.rs

use crate::{
    engine::{
        ast::{Node, NodeValue, Span},
        error::ParseError,
        lexer::Token,
    },
};

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&Token> {
        let tok = self.tokens.get(self.pos);
        self.pos += 1;
        tok
    }

    fn expect_lbrace(&mut self) -> Result<usize, ParseError> {
        let token_result = self.advance(); // Mutable borrow for advance ends here.

        match token_result {
            Some(Token::LBrace(pos)) => Ok(*pos),
            Some(tok) => {
                // Calculate the span using an immutable borrow
                let span = token_span_static(tok); 
                
                // Return the error
                Err(ParseError::UnexpectedToken { 
                    expected: "{", 
                    found: span // Use the calculated span
                })
            }
            None => Err(ParseError::UnexpectedEOF { 
                span: Span { start: self.pos, end: self.pos } 
            }),
        }
    }

    fn expect_rbrace(&mut self) -> Result<usize, ParseError> {
        let token_result = self.advance(); // Mutable borrow drops here

        match token_result {
            Some(Token::RBrace(pos)) => Ok(*pos),
            Some(tok) => {
                let span = token_span_static(tok); 

                Err(ParseError::UnexpectedToken { 
                    expected: "}", 
                    found: span 
                })
            },
            None => Err(ParseError::UnexpectedEOF { 
                span: Span { start: self.pos, end: self.pos } 
            }),
        }
    }

    fn parse_identifier(&mut self) -> Option<(String, usize)> {
        match self.advance()? {
            Token::Identifier(s, pos) => Some((s.clone(), *pos)),
            _ => None,
        }
    }

    fn parse_value(&mut self) -> Result<NodeValue, ParseError> {
        let token = self.clone_peek().ok_or(ParseError::UnexpectedEOF {
            span: Span { start: self.pos, end: self.pos },
        })?;

        // let owned_tok = token.clone();

        match token {
            Token::LBrace(_) => {
                let block = self.parse_block()?;
                Ok(NodeValue::Block(block))
            }

            Token::StringLiteral(_, start) => {
                if let Token::StringLiteral(s, pos) = self.advance().unwrap() {
                    Ok(NodeValue::String(s.clone()))
                } else { unreachable!() }
            }

            Token::Number(n, start) => {
                if let Token::Number(n, pos) = self.advance().unwrap() {
                    Ok(NodeValue::Number(*n))
                } else { unreachable!() }
            }

            Token::Float(f, start) => {
                if let Token::Float(f, pos) = self.advance().unwrap() {
                    Ok(NodeValue::Float(*f))
                } else { unreachable!() }
            }

            Token::Bool(b, start) => {
                if let Token::Bool(b, pos) = self.advance().unwrap() {
                    Ok(NodeValue::Bool(*b))
                } else { unreachable!() }
            }

            tok => Err(ParseError::UnexpectedToken {
                expected: "value",
                found: self.token_span(&tok)
            }),
        }
    }

    fn parse_block(&mut self) -> Result<Vec<Node>, ParseError> {
        let open_pos = self.expect_lbrace()?;

        let mut nodes = Vec::new();

        loop {
            let token = self.clone_peek().ok_or(ParseError::UnexpectedEOF {
                span: Span { start: self.pos, end: self.pos }
            })?;

            match token {
                Token::RBrace(close_pos) => {
                    self.advance();
                    return Ok(nodes);
                }

                Token::Identifier(_, _) => {
                    let node = self.parse_node()?;
                    nodes.push(node);
                }

                _ => {
                    self.advance(); // skip unexpected
                }
            }
        }
    }

    fn parse_node(&mut self) -> Result<Node, ParseError> {
        let (key, key_pos) = match self.advance() {
            Some(Token::Identifier(s, pos)) => (s.clone(), *pos),
            Some(tok) => {
                let span = token_span_static(tok); 

                return Err(ParseError::UnexpectedToken {
                    expected: "identifier",
                    found: span,
                })
            }
            None => {
                return Err(ParseError::UnexpectedEOF {
                    span: Span { start: self.pos, end: self.pos },
                })
            }
        };

        let value = self.parse_value()?;

        // End the AST span at the actual final token of this node, not at
        // the beginning of the next sibling. Otherwise inter-property
        // whitespace belongs to the *previous* scalar value in LSP context,
        // and schema key completions silently disappear on the next line.
        let end = self.pos
            .checked_sub(1)
            .and_then(|index| self.tokens.get(index))
            .map(token_end_static)
            .unwrap_or_else(|| key_pos + key.chars().count());

        let span = Span {
            start: key_pos,
            end,
        };

        Ok(Node { key, value, span })
    }

    fn token_span(&self, tok: &Token) -> Span {
        match tok {
            Token::LBrace(pos)
            | Token::RBrace(pos)
            | Token::Identifier(_, pos)
            | Token::StringLiteral(_, pos)
            | Token::Number(_, pos)
            | Token::Float(_, pos)
            | Token::Bool(_, pos) => Span { start: *pos, end: *pos + 1 },
        }
    }

    pub fn parse(mut self) -> Result<Vec<Node>, ParseError> {
        let mut out = Vec::new();

        loop {
            let token = match self.peek().cloned() {
                Some(t) => t,
                None => break,
            };

            match token {
                Token::LBrace(_) => {
                    let block = self.parse_block()?;
                    out.extend(block);
                }
                _ => break,
            }
        }

        Ok(out)
    }
}


impl Parser {
    fn clone_peek(&self) -> Option<Token> {
        self.peek().cloned()
    }
}



pub fn token_start_static(tok: &Token) -> usize {
    match tok {
        Token::LBrace(pos)
        | Token::RBrace(pos)
        | Token::Identifier(_, pos)
        | Token::StringLiteral(_, pos)
        | Token::Number(_, pos)
        | Token::Float(_, pos)
        | Token::Bool(_, pos) => *pos,
    }
}

pub fn token_end_static(tok: &Token) -> usize {
    match tok {
        Token::LBrace(pos) | Token::RBrace(pos) => *pos + 1,
        Token::Identifier(value, pos) => *pos + value.chars().count(),
        Token::StringLiteral(value, pos) => *pos + value.chars().count() + 2,
        Token::Number(value, pos) => *pos + value.to_string().chars().count(),
        Token::Float(value, pos) => *pos + value.to_string().chars().count(),
        Token::Bool(value, pos) => *pos + if *value { 4 } else { 5 },
    }
}

pub fn token_span_static(tok: &Token) -> Span {
    match tok {
        Token::LBrace(pos)
        | Token::RBrace(pos)
        | Token::Identifier(_, pos)
        | Token::StringLiteral(_, pos)
        | Token::Number(_, pos)
        | Token::Float(_, pos)
        | Token::Bool(_, pos) => Span { start: *pos, end: *pos + 1 },
    }
}