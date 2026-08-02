use std::collections::HashSet;

use herta_core::{HbError, HbResult};
use serde_json::{Number, Value};

const MAX_FILTER_BYTES: usize = 4096;
const MAX_CONDITIONS: usize = 64;
const MAX_DEPTH: usize = 8;

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Ident(String),
    Literal(Value),
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
    And,
    Or,
    In,
    Contains,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompareOp {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
    In,
    Contains,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogicalOp {
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq)]
enum Expr {
    Compare {
        field: String,
        op: CompareOp,
        value: Value,
    },
    Logical {
        left: Box<Expr>,
        op: LogicalOp,
        right: Box<Expr>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledFilter {
    pub sql: String,
    pub bindings: Vec<(String, Value)>,
}

pub fn compile_filter(input: &str, allowed_fields: &HashSet<String>) -> HbResult<CompiledFilter> {
    if input.len() > MAX_FILTER_BYTES {
        return Err(HbError::InvalidFilter(format!(
            "filter exceeds {MAX_FILTER_BYTES} bytes"
        )));
    }
    let tokens = tokenize(input)?;
    let mut parser = Parser {
        tokens,
        pos: 0,
        conditions: 0,
    };
    let expression = parser.parse_expression(0)?;
    if parser.peek() != &Token::End {
        return Err(HbError::InvalidFilter("unexpected trailing input".into()));
    }
    let mut bindings = Vec::new();
    let sql = compile_expr(&expression, allowed_fields, &mut bindings)?;
    Ok(CompiledFilter { sql, bindings })
}

fn tokenize(input: &str) -> HbResult<Vec<Token>> {
    let chars: Vec<char> = input.chars().collect();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        match chars[index] {
            ch if ch.is_whitespace() => index += 1,
            '(' => {
                tokens.push(Token::LParen);
                index += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                index += 1;
            }
            '[' => {
                tokens.push(Token::LBracket);
                index += 1;
            }
            ']' => {
                tokens.push(Token::RBracket);
                index += 1;
            }
            ',' => {
                tokens.push(Token::Comma);
                index += 1;
            }
            '=' => {
                tokens.push(Token::Eq);
                index += 1;
            }
            '!' if chars.get(index + 1) == Some(&'=') => {
                tokens.push(Token::Ne);
                index += 2;
            }
            '>' if chars.get(index + 1) == Some(&'=') => {
                tokens.push(Token::Ge);
                index += 2;
            }
            '>' => {
                tokens.push(Token::Gt);
                index += 1;
            }
            '<' if chars.get(index + 1) == Some(&'=') => {
                tokens.push(Token::Le);
                index += 2;
            }
            '<' => {
                tokens.push(Token::Lt);
                index += 1;
            }
            quote @ ('\'' | '"') => {
                let (value, next) = read_string(&chars, index + 1, quote)?;
                tokens.push(Token::Literal(Value::String(value)));
                index = next;
            }
            '-' | '0'..='9' => {
                let start = index;
                index += 1;
                while chars.get(index).is_some_and(|ch| {
                    ch.is_ascii_digit() || matches!(ch, '.' | 'e' | 'E' | '+' | '-')
                }) {
                    index += 1;
                }
                let raw: String = chars[start..index].iter().collect();
                let number: Number = raw
                    .parse()
                    .map_err(|_| HbError::InvalidFilter(format!("invalid number '{raw}'")))?;
                tokens.push(Token::Literal(Value::Number(number)));
            }
            ch if ch.is_ascii_alphabetic() || ch == '_' => {
                let start = index;
                index += 1;
                while chars
                    .get(index)
                    .is_some_and(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
                {
                    index += 1;
                }
                let word: String = chars[start..index].iter().collect();
                tokens.push(match word.to_ascii_uppercase().as_str() {
                    "AND" => Token::And,
                    "OR" => Token::Or,
                    "IN" => Token::In,
                    "CONTAINS" => Token::Contains,
                    "TRUE" => Token::Literal(Value::Bool(true)),
                    "FALSE" => Token::Literal(Value::Bool(false)),
                    "NULL" | "NONE" => Token::Literal(Value::Null),
                    _ => Token::Ident(word),
                });
            }
            ch => {
                return Err(HbError::InvalidFilter(format!(
                    "unexpected character '{ch}'"
                )));
            }
        }
    }
    tokens.push(Token::End);
    Ok(tokens)
}

fn read_string(chars: &[char], mut index: usize, quote: char) -> HbResult<(String, usize)> {
    let mut value = String::new();
    while index < chars.len() {
        match chars[index] {
            ch if ch == quote => return Ok((value, index + 1)),
            '\\' => {
                index += 1;
                let escaped = chars
                    .get(index)
                    .ok_or_else(|| HbError::InvalidFilter("unterminated escape".into()))?;
                value.push(*escaped);
                index += 1;
            }
            ch => {
                value.push(ch);
                index += 1;
            }
        }
    }
    Err(HbError::InvalidFilter("unterminated string".into()))
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    conditions: usize,
}

impl Parser {
    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::End)
    }

    fn next(&mut self) -> Token {
        let token = self.peek().clone();
        self.pos += 1;
        token
    }

    fn parse_expression(&mut self, depth: usize) -> HbResult<Expr> {
        if depth > MAX_DEPTH {
            return Err(HbError::InvalidFilter(format!(
                "filter nesting exceeds {MAX_DEPTH} levels"
            )));
        }
        let mut expression = self.parse_and(depth)?;
        while matches!(self.peek(), Token::Or) {
            self.next();
            let right = self.parse_and(depth)?;
            expression = Expr::Logical {
                left: Box::new(expression),
                op: LogicalOp::Or,
                right: Box::new(right),
            };
        }
        Ok(expression)
    }

    fn parse_and(&mut self, depth: usize) -> HbResult<Expr> {
        let mut expression = self.parse_primary(depth)?;
        while matches!(self.peek(), Token::And) {
            self.next();
            let right = self.parse_primary(depth)?;
            expression = Expr::Logical {
                left: Box::new(expression),
                op: LogicalOp::And,
                right: Box::new(right),
            };
        }
        Ok(expression)
    }

    fn parse_primary(&mut self, depth: usize) -> HbResult<Expr> {
        if matches!(self.peek(), Token::LParen) {
            self.next();
            let expression = self.parse_expression(depth + 1)?;
            if !matches!(self.next(), Token::RParen) {
                return Err(HbError::InvalidFilter("expected ')'".into()));
            }
            return Ok(expression);
        }
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> HbResult<Expr> {
        self.conditions += 1;
        if self.conditions > MAX_CONDITIONS {
            return Err(HbError::InvalidFilter(format!(
                "filter exceeds {MAX_CONDITIONS} conditions"
            )));
        }
        let Token::Ident(field) = self.next() else {
            return Err(HbError::InvalidFilter("expected field name".into()));
        };
        let op = match self.next() {
            Token::Eq => CompareOp::Eq,
            Token::Ne => CompareOp::Ne,
            Token::Gt => CompareOp::Gt,
            Token::Ge => CompareOp::Ge,
            Token::Lt => CompareOp::Lt,
            Token::Le => CompareOp::Le,
            Token::In => CompareOp::In,
            Token::Contains => CompareOp::Contains,
            _ => {
                return Err(HbError::InvalidFilter(
                    "expected comparison operator".into(),
                ));
            }
        };
        let value = self.parse_value()?;
        if op == CompareOp::In && !value.is_array() {
            return Err(HbError::InvalidFilter(
                "IN requires an array literal".into(),
            ));
        }
        Ok(Expr::Compare { field, op, value })
    }

    fn parse_value(&mut self) -> HbResult<Value> {
        if matches!(self.peek(), Token::LBracket) {
            self.next();
            let mut values = Vec::new();
            if matches!(self.peek(), Token::RBracket) {
                self.next();
                return Ok(Value::Array(values));
            }
            loop {
                let Token::Literal(value) = self.next() else {
                    return Err(HbError::InvalidFilter(
                        "array values must be literals".into(),
                    ));
                };
                values.push(value);
                match self.next() {
                    Token::Comma => {}
                    Token::RBracket => break,
                    _ => return Err(HbError::InvalidFilter("expected ',' or ']'".into())),
                }
            }
            return Ok(Value::Array(values));
        }
        let Token::Literal(value) = self.next() else {
            return Err(HbError::InvalidFilter("expected literal value".into()));
        };
        Ok(value)
    }
}

fn compile_expr(
    expression: &Expr,
    allowed_fields: &HashSet<String>,
    bindings: &mut Vec<(String, Value)>,
) -> HbResult<String> {
    match expression {
        Expr::Compare { field, op, value } => {
            if !allowed_fields.contains(field) {
                return Err(HbError::InvalidFilter(format!("unknown field '{field}'")));
            }
            let name = format!("filter_{}", bindings.len());
            bindings.push((name.clone(), value.clone()));
            let operator = match op {
                CompareOp::Eq => "=",
                CompareOp::Ne => "!=",
                CompareOp::Gt => ">",
                CompareOp::Ge => ">=",
                CompareOp::Lt => "<",
                CompareOp::Le => "<=",
                CompareOp::In => "IN",
                CompareOp::Contains => "CONTAINS",
            };
            Ok(format!("`{field}` {operator} ${name}"))
        }
        Expr::Logical { left, op, right } => {
            let left = compile_expr(left, allowed_fields, bindings)?;
            let right = compile_expr(right, allowed_fields, bindings)?;
            let op = match op {
                LogicalOp::And => "AND",
                LogicalOp::Or => "OR",
            };
            Ok(format!("({left} {op} {right})"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_nested_filter_with_bindings() {
        let allowed = ["status", "score", "tags"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        let compiled = compile_filter(
            "(status = 'active' AND score >= 10) OR tags CONTAINS 'rust'",
            &allowed,
        )
        .unwrap();
        assert_eq!(compiled.bindings.len(), 3);
        assert!(!compiled.sql.contains("active"));
        assert!(compiled.sql.contains("$filter_0"));
    }

    #[test]
    fn supports_in_arrays() {
        let allowed = ["status"].into_iter().map(str::to_owned).collect();
        let compiled = compile_filter("status IN ['draft', 'active']", &allowed).unwrap();
        assert_eq!(compiled.bindings[0].1.as_array().unwrap().len(), 2);
    }

    #[test]
    fn rejects_functions_and_unknown_fields() {
        let allowed = ["status"].into_iter().map(str::to_owned).collect();
        assert!(compile_filter("status = crypto::random_uuid()", &allowed).is_err());
        assert!(compile_filter("secret = 'x'", &allowed).is_err());
    }
}
