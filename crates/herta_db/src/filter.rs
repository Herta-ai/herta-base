use herta_core::{HbError, HbResult};
use serde_json::{Number, Value};
use surrealdb::types::{RecordId, ToSql, Value as SurrealValue};

use crate::{
    models::{CollectionDef, FieldDef, FieldType, relation_is_many},
    record::parse_record_id,
};

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
    pub live_sql: String,
    pub bindings: Vec<(String, SurrealValue)>,
}

pub fn compile_filter(input: &str, schema: &CollectionDef) -> HbResult<CompiledFilter> {
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
    let sql = compile_expr(&expression, schema, &mut bindings)?;
    let live_sql = compile_live_expr(&expression, schema)?;
    Ok(CompiledFilter {
        sql,
        live_sql,
        bindings,
    })
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
    schema: &CollectionDef,
    bindings: &mut Vec<(String, SurrealValue)>,
) -> HbResult<String> {
    match expression {
        Expr::Compare { field, op, value } => {
            let definition = schema
                .fields
                .iter()
                .find(|candidate| candidate.name == *field);
            if definition.is_none() && !SYSTEM_FIELDS.contains(&field.as_str()) {
                return Err(HbError::InvalidFilter(format!("unknown field '{field}'")));
            }
            let name = format!("filter_{}", bindings.len());
            let value = match definition {
                Some(definition) if definition.field_type == FieldType::Relation => {
                    compile_relation_value(definition, *op, value)?
                }
                _ => SurrealValue::from_t(value.clone()),
            };
            bindings.push((name.clone(), value));
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
            let left = compile_expr(left, schema, bindings)?;
            let right = compile_expr(right, schema, bindings)?;
            let op = match op {
                LogicalOp::And => "AND",
                LogicalOp::Or => "OR",
            };
            Ok(format!("({left} {op} {right})"))
        }
    }
}

const SYSTEM_FIELDS: [&str; 4] = ["id", "created_at", "updated_at", "deleted_at"];

fn compile_relation_value(
    field: &FieldDef,
    op: CompareOp,
    value: &Value,
) -> HbResult<SurrealValue> {
    let many = relation_is_many(field.options.as_ref());
    let allowed_operator = if many {
        op == CompareOp::Contains
    } else {
        matches!(op, CompareOp::Eq | CompareOp::Ne | CompareOp::In)
    };
    if !allowed_operator {
        let expected = if many { "CONTAINS" } else { "=, !=, or IN" };
        return Err(HbError::InvalidFilter(format!(
            "relation field '{}' requires {expected}",
            field.name
        )));
    }

    let target = field
        .options
        .as_ref()
        .and_then(|options| options.get("collection"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            HbError::InvalidFilter(format!(
                "relation field '{}' has no target collection",
                field.name
            ))
        })?;

    if op == CompareOp::In {
        let values = value.as_array().ok_or_else(|| {
            HbError::InvalidFilter(format!(
                "relation field '{}' requires an array for IN",
                field.name
            ))
        })?;
        let records = values
            .iter()
            .map(|value| parse_relation_id(&field.name, target, value))
            .collect::<HbResult<Vec<_>>>()?;
        Ok(SurrealValue::from_t(records))
    } else {
        Ok(SurrealValue::from_t(parse_relation_id(
            &field.name,
            target,
            value,
        )?))
    }
}

fn compile_live_expr(expression: &Expr, schema: &CollectionDef) -> HbResult<String> {
    match expression {
        Expr::Compare { field, op, value } => {
            let definition = schema
                .fields
                .iter()
                .find(|candidate| candidate.name == *field);
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
            let Some(definition) =
                definition.filter(|field| field.field_type == FieldType::Relation)
            else {
                return Ok(format!(
                    "`{field}` {operator} {}",
                    SurrealValue::from_t(value.clone()).to_sql()
                ));
            };
            let target = definition
                .options
                .as_ref()
                .and_then(|options| options.get("collection"))
                .and_then(Value::as_str)
                .expect("relation target was validated while compiling bindings");
            let relation_string = |value: &Value| -> HbResult<SurrealValue> {
                let record = parse_relation_id(&definition.name, target, value)?;
                Ok(SurrealValue::from_t(record.to_sql()))
            };
            let right = if *op == CompareOp::In {
                let values = value
                    .as_array()
                    .expect("relation IN values were validated")
                    .iter()
                    .map(relation_string)
                    .collect::<HbResult<Vec<_>>>()?;
                SurrealValue::from_t(values).to_sql()
            } else {
                relation_string(value)?.to_sql()
            };
            let left = if relation_is_many(definition.options.as_ref()) {
                format!("array::map(`{field}`, |$relation| type::string($relation))")
            } else {
                format!("type::string(`{field}`)")
            };
            Ok(format!("{left} {operator} {right}"))
        }
        Expr::Logical { left, op, right } => {
            let left = compile_live_expr(left, schema)?;
            let right = compile_live_expr(right, schema)?;
            let operator = match op {
                LogicalOp::And => "AND",
                LogicalOp::Or => "OR",
            };
            Ok(format!("({left} {operator} {right})"))
        }
    }
}

fn parse_relation_id(field: &str, target: &str, value: &Value) -> HbResult<RecordId> {
    let raw = value.as_str().ok_or_else(|| {
        HbError::InvalidFilter(format!(
            "relation field '{field}' requires record ID string values"
        ))
    })?;
    let record = parse_record_id(raw).map_err(|_| {
        HbError::InvalidFilter(format!(
            "relation field '{field}' contains invalid record ID '{raw}'"
        ))
    })?;
    if record.table.as_str() != target {
        return Err(HbError::InvalidFilter(format!(
            "relation field '{field}' targets '{target}', not '{}'",
            record.table.as_str()
        )));
    }
    Ok(record)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema(fields: Vec<FieldDef>) -> CollectionDef {
        CollectionDef {
            name: "tasks".into(),
            collection_type: crate::models::CollectionType::Base,
            schema_mode: crate::models::SchemaMode::Strict,
            fields,
            indexes: Vec::new(),
            rules: Default::default(),
        }
    }

    fn field(name: &str, field_type: FieldType, options: Option<Value>) -> FieldDef {
        FieldDef {
            name: name.into(),
            field_type,
            required: false,
            options,
        }
    }

    #[test]
    fn compiles_nested_filter_with_bindings() {
        let schema = schema(vec![
            field("status", FieldType::Text, None),
            field("score", FieldType::Number, None),
            field("tags", FieldType::Json, None),
        ]);
        let compiled = compile_filter(
            "(status = 'active' AND score >= 10) OR tags CONTAINS 'rust'",
            &schema,
        )
        .unwrap();
        assert_eq!(compiled.bindings.len(), 3);
        assert!(!compiled.sql.contains("active"));
        assert!(compiled.sql.contains("$filter_0"));
    }

    #[test]
    fn supports_in_arrays() {
        let schema = schema(vec![field("status", FieldType::Text, None)]);
        let compiled = compile_filter("status IN ['draft', 'active']", &schema).unwrap();
        let values = compiled.bindings[0]
            .1
            .clone()
            .into_t::<Vec<String>>()
            .unwrap();
        assert_eq!(values, ["draft", "active"]);
    }

    #[test]
    fn rejects_functions_and_unknown_fields() {
        let schema = schema(vec![field("status", FieldType::Text, None)]);
        assert!(compile_filter("status = crypto::random_uuid()", &schema).is_err());
        assert!(compile_filter("secret = 'x'", &schema).is_err());
    }

    #[test]
    fn compiles_single_and_multi_relation_bindings() {
        let schema = schema(vec![
            field(
                "workspace",
                FieldType::Relation,
                Some(serde_json::json!({"collection": "workspaces", "maxSelect": 1})),
            ),
            field(
                "assignees",
                FieldType::Relation,
                Some(serde_json::json!({"collection": "users", "maxSelect": 10})),
            ),
        ]);

        let compiled = compile_filter(
            "workspace IN ['workspaces:one', 'workspaces:two'] AND assignees CONTAINS 'users:a'",
            &schema,
        )
        .unwrap();
        let workspaces = compiled.bindings[0]
            .1
            .clone()
            .into_t::<Vec<RecordId>>()
            .unwrap();
        assert_eq!(workspaces[0], RecordId::new("workspaces", "one"));
        assert_eq!(
            compiled.bindings[1].1.clone().into_t::<RecordId>().unwrap(),
            RecordId::new("users", "a")
        );
    }

    #[test]
    fn rejects_invalid_relation_filters_but_keeps_plain_strings() {
        let schema = schema(vec![
            field("label", FieldType::Text, None),
            field(
                "workspace",
                FieldType::Relation,
                Some(serde_json::json!({"collection": "workspaces", "maxSelect": 1})),
            ),
            field(
                "assignees",
                FieldType::Relation,
                Some(serde_json::json!({"collection": "users", "maxSelect": 10})),
            ),
        ]);

        assert!(compile_filter("workspace CONTAINS 'workspaces:one'", &schema).is_err());
        assert!(compile_filter("assignees = 'users:one'", &schema).is_err());
        assert!(compile_filter("assignees IN ['users:one']", &schema).is_err());
        assert!(compile_filter("workspace = 'users:one'", &schema).is_err());
        assert!(compile_filter("workspace = 'not-an-id'", &schema).is_err());
        assert!(compile_filter("workspace = 1", &schema).is_err());

        let compiled = compile_filter("label = 'users:one'", &schema).unwrap();
        assert_eq!(
            compiled.bindings[0].1.clone().into_t::<String>().unwrap(),
            "users:one"
        );
        assert_eq!(compiled.live_sql, "`label` = 'users:one'");
    }
}
