//! Shared expression lexer and precedence parser. Core applies its own capability
//! and type checks to the resulting AST; compatibility entry points use the same syntax.
use crate::error::{ParseError, Result};
use corint_decision_model::ast::{Expression, Operator, UnaryOperator};
use corint_decision_model::Value;

pub struct ExpressionParser;

impl ExpressionParser {
    pub fn parse(input: &str) -> Result<Expression> {
        let mut parser = Parser {
            tokens: lex(input)?,
            position: 0,
            depth: 0,
        };
        let expression = parser.expression(1)?;
        if parser.peek().is_some() {
            return Err(invalid("Unexpected trailing token"));
        }
        Ok(expression)
    }
}

fn invalid(message: impl Into<String>) -> ParseError {
    ParseError::InvalidExpression(message.into())
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Word(String),
    Number(String),
    String(String),
    Op(Operator),
    Not,
    Dot,
    Comma,
    Open,
    Close,
    ArrayOpen,
    ArrayClose,
}

// All slicing uses character boundaries. Quoted content is consumed as a single
// token before recognizing punctuation, keywords or operators.
fn lex(input: &str) -> Result<Vec<Token>> {
    let mut chars = input.char_indices().peekable();
    let mut tokens = Vec::new();
    while let Some((start, c)) = chars.next() {
        let token = match c {
            c if c.is_whitespace() => continue,
            '\'' | '"' => {
                let quote = c;
                let mut value = String::new();
                let mut closed = false;
                while let Some((_, c)) = chars.next() {
                    if c == quote {
                        closed = true;
                        break;
                    }
                    if c != '\\' {
                        if c.is_control() {
                            return Err(invalid("Control characters in strings must be escaped"));
                        }
                        value.push(c);
                        continue;
                    }
                    let (_, escaped) = chars
                        .next()
                        .ok_or_else(|| invalid("Incomplete string escape"))?;
                    value.push(match escaped {
                        '\\' => '\\',
                        '"' => '"',
                        '\'' => '\'',
                        '/' => '/',
                        'n' => '\n',
                        'r' => '\r',
                        't' => '\t',
                        'b' => '\u{0008}',
                        'f' => '\u{000c}',
                        'u' => {
                            let first = hex_quad(&mut chars)?;
                            let scalar = if (0xd800..=0xdbff).contains(&first) {
                                if chars.next().map(|(_, c)| c) != Some('\\')
                                    || chars.next().map(|(_, c)| c) != Some('u')
                                {
                                    return Err(invalid("Missing low Unicode surrogate"));
                                }
                                let second = hex_quad(&mut chars)?;
                                if !(0xdc00..=0xdfff).contains(&second) {
                                    return Err(invalid("Invalid low Unicode surrogate"));
                                }
                                0x10000 + ((first - 0xd800) << 10) + second - 0xdc00
                            } else {
                                first
                            };
                            char::from_u32(scalar)
                                .ok_or_else(|| invalid("Invalid Unicode escape"))?
                        }
                        _ => return Err(invalid(format!("Unknown string escape: \\{escaped}"))),
                    });
                }
                if !closed {
                    return Err(invalid("Unterminated string"));
                }
                Token::String(value)
            }
            c if c.is_ascii_digit() && tokens.last() == Some(&Token::Dot) => {
                while chars
                    .peek()
                    .is_some_and(|(_, c)| c.is_alphanumeric() || *c == '_')
                {
                    chars.next();
                }
                let end = chars.peek().map(|(i, _)| *i).unwrap_or(input.len());
                Token::Word(input[start..end].into())
            }
            c if c.is_ascii_digit() => {
                while chars.peek().is_some_and(|(_, c)| c.is_ascii_digit()) {
                    chars.next();
                }
                if chars.peek().is_some_and(|(_, c)| *c == '.') {
                    chars.next();
                    while chars.peek().is_some_and(|(_, c)| c.is_ascii_digit()) {
                        chars.next();
                    }
                }
                if chars.peek().is_some_and(|(_, c)| matches!(c, 'e' | 'E')) {
                    chars.next();
                    if chars.peek().is_some_and(|(_, c)| matches!(c, '+' | '-')) {
                        chars.next();
                    }
                    let mut digits = 0;
                    while chars.peek().is_some_and(|(_, c)| c.is_ascii_digit()) {
                        chars.next();
                        digits += 1;
                    }
                    if digits == 0 {
                        return Err(invalid("Missing exponent digits"));
                    }
                }
                let end = chars.peek().map(|(i, _)| *i).unwrap_or(input.len());
                Token::Number(input[start..end].into())
            }
            c if c.is_alphabetic() || c == '_' => {
                while chars
                    .peek()
                    .is_some_and(|(_, c)| c.is_alphanumeric() || *c == '_')
                {
                    chars.next();
                }
                let end = chars.peek().map(|(i, _)| *i).unwrap_or(input.len());
                Token::Word(input[start..end].into())
            }
            '.' => Token::Dot,
            ',' => Token::Comma,
            '(' => Token::Open,
            ')' => Token::Close,
            '[' => Token::ArrayOpen,
            ']' => Token::ArrayClose,
            '+' => Token::Op(Operator::Add),
            '-' => Token::Op(Operator::Sub),
            '*' => Token::Op(Operator::Mul),
            '/' => Token::Op(Operator::Div),
            '%' => Token::Op(Operator::Mod),
            '=' | '!' | '<' | '>' | '&' | '|' => {
                let paired = chars
                    .peek()
                    .is_some_and(|(_, next)| *next == if c == '&' || c == '|' { c } else { '=' });
                if paired {
                    chars.next();
                }
                match (c, paired) {
                    ('=', true) => Token::Op(Operator::Eq),
                    ('!', true) => Token::Op(Operator::Ne),
                    ('<', true) => Token::Op(Operator::Le),
                    ('>', true) => Token::Op(Operator::Ge),
                    ('&', true) => Token::Op(Operator::And),
                    ('|', true) => Token::Op(Operator::Or),
                    ('<', false) => Token::Op(Operator::Lt),
                    ('>', false) => Token::Op(Operator::Gt),
                    ('!', false) => Token::Not,
                    _ => return Err(invalid(format!("Invalid operator at byte {start}"))),
                }
            }
            _ => {
                return Err(invalid(format!(
                    "Unexpected character at byte {start}: {c}"
                )))
            }
        };
        if tokens.len() >= 4096 {
            return Err(invalid("Expression exceeds 4096 tokens"));
        }
        tokens.push(token);
    }
    Ok(tokens)
}

fn hex_quad(chars: &mut impl Iterator<Item = (usize, char)>) -> Result<u32> {
    let mut value = 0;
    for _ in 0..4 {
        let digit = chars
            .next()
            .and_then(|(_, c)| c.to_digit(16))
            .ok_or_else(|| invalid("Unicode escapes require four hexadecimal digits"))?;
        value = value * 16 + digit;
    }
    Ok(value)
}

struct Parser {
    tokens: Vec<Token>,
    position: usize,
    depth: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }
    fn take(&mut self) -> Result<Token> {
        let token = self
            .peek()
            .cloned()
            .ok_or_else(|| invalid("Expected an expression"))?;
        self.position += 1;
        Ok(token)
    }
    fn expect(&mut self, expected: Token) -> Result<()> {
        if self.take()? != expected {
            return Err(invalid(format!("Expected {expected:?}")));
        }
        Ok(())
    }

    fn binary(&self) -> Option<(Operator, u8, usize)> {
        let (op, width) = match self.peek()? {
            Token::Op(op) => (*op, 1),
            Token::Word(word) => (
                match word.as_str() {
                    "in" => Operator::In,
                    "not_in" => Operator::NotIn,
                    "not"
                        if self.tokens.get(self.position + 1)
                            == Some(&Token::Word("in".into())) =>
                    {
                        return Some((Operator::NotIn, 4, 2))
                    }
                    "contains" => Operator::Contains,
                    "starts_with" => Operator::StartsWith,
                    "ends_with" => Operator::EndsWith,
                    "regex" => Operator::Regex,
                    _ => return None,
                },
                1,
            ),
            _ => return None,
        };
        let precedence = match op {
            Operator::Or => 1,
            Operator::And => 2,
            Operator::Eq | Operator::Ne => 3,
            Operator::Add | Operator::Sub => 5,
            Operator::Mul | Operator::Div | Operator::Mod => 6,
            _ => 4,
        };
        Some((op, precedence, width))
    }

    fn expression(&mut self, minimum: u8) -> Result<Expression> {
        self.depth += 1;
        if self.depth > 128 {
            return Err(invalid("Expression nesting exceeds 128"));
        }
        let result = self.expression_inner(minimum);
        self.depth -= 1;
        if let Ok(expr) = &result {
            check_ast_depth(expr)?;
        }
        result
    }

    fn expression_inner(&mut self, minimum: u8) -> Result<Expression> {
        let mut left = self.primary()?;
        while let Some((mut op, precedence, width)) = self.binary() {
            if precedence < minimum {
                break;
            }
            self.position += width;
            let mut right = self.expression(precedence + 1)?;
            if matches!(op, Operator::In | Operator::NotIn) {
                if let Expression::FieldAccess(fields) = &right {
                    if fields.len() >= 2 && fields[0] == "list" {
                        right = Expression::ListReference {
                            list_id: fields[1..].join("."),
                        };
                        op = if op == Operator::In {
                            Operator::InList
                        } else {
                            Operator::NotInList
                        };
                    }
                }
            }
            left = Expression::binary(left, op, right);
            check_ast_depth(&left)?;
        }
        Ok(left)
    }

    fn primary(&mut self) -> Result<Expression> {
        match self.take()? {
            Token::Not => Ok(Expression::unary(UnaryOperator::Not, self.expression(7)?)),
            Token::Op(Operator::Sub) => {
                let operand = self.expression(7)?;
                Ok(match operand {
                    Expression::Literal(Value::Number(n)) => Expression::literal(Value::Number(-n)),
                    other => Expression::unary(UnaryOperator::Negate, other),
                })
            }
            Token::Number(text) => Ok(Expression::literal(Value::Number(
                text.parse().map_err(|_| invalid("Invalid number"))?,
            ))),
            Token::String(value) => Ok(Expression::literal(Value::String(value))),
            Token::Open => {
                let value = self.expression(1)?;
                self.expect(Token::Close)?;
                Ok(value)
            }
            Token::ArrayOpen => {
                let mut values = Vec::new();
                if self.peek() != Some(&Token::ArrayClose) {
                    loop {
                        let Expression::Literal(value) = self.expression(1)? else {
                            return Err(invalid("Array elements must be literals"));
                        };
                        values.push(value);
                        if self.peek() != Some(&Token::Comma) {
                            break;
                        }
                        self.position += 1;
                    }
                }
                self.expect(Token::ArrayClose)?;
                Ok(Expression::literal(Value::Array(values)))
            }
            Token::Word(name) => {
                if self.peek() == Some(&Token::Open) {
                    self.position += 1;
                    let mut args = Vec::new();
                    if self.peek() != Some(&Token::Close) {
                        loop {
                            args.push(self.expression(1)?);
                            if self.peek() != Some(&Token::Comma) {
                                break;
                            }
                            self.position += 1;
                        }
                    }
                    self.expect(Token::Close)?;
                    return Ok(Expression::function_call(name, args));
                }
                let mut fields = vec![name];
                while self.peek() == Some(&Token::Dot) {
                    self.position += 1;
                    fields.push(match self.take()? {
                        Token::Word(word) => word,
                        Token::Number(number) if number.chars().all(|c| c.is_ascii_digit()) => {
                            number
                        }
                        _ => return Err(invalid("Expected field name after '.'")),
                    });
                }
                if fields.len() == 1 {
                    match fields[0].as_str() {
                        "true" => return Ok(Expression::literal(Value::Bool(true))),
                        "false" => return Ok(Expression::literal(Value::Bool(false))),
                        "null" => return Ok(Expression::literal(Value::Null)),
                        _ => (),
                    }
                }
                if fields.len() >= 2 && matches!(fields[0].as_str(), "result" | "results") {
                    return Ok(Expression::ResultAccess {
                        ruleset_id: if fields.len() > 2 {
                            Some(fields[1].clone())
                        } else {
                            None
                        },
                        field: fields[if fields.len() > 2 { 2 } else { 1 }..].join("."),
                    });
                }
                Ok(Expression::field_access(fields))
            }
            _ => Err(invalid(
                "Expected a literal, field, function or parenthesized expression",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precedence_and_associativity_are_explicit() {
        let atom = |s: &str| Expression::field_access(vec![s.into()]);
        assert_eq!(
            ExpressionParser::parse("a || b && c").unwrap(),
            Expression::binary(
                atom("a"),
                Operator::Or,
                Expression::binary(atom("b"), Operator::And, atom("c"))
            )
        );
        assert_eq!(
            ExpressionParser::parse("a - b - c * d").unwrap(),
            Expression::binary(
                Expression::binary(atom("a"), Operator::Sub, atom("b")),
                Operator::Sub,
                Expression::binary(atom("c"), Operator::Mul, atom("d"))
            )
        );
        assert_eq!(
            ExpressionParser::parse("-event.amount").unwrap(),
            Expression::unary(
                UnaryOperator::Negate,
                Expression::field_access(vec!["event".into(), "amount".into()])
            )
        );
        for (text, number) in [("-1", -1.0), ("1e-3", 0.001), ("-1.5E+2", -150.0)] {
            assert_eq!(
                ExpressionParser::parse(text).unwrap(),
                Expression::literal(Value::Number(number))
            );
        }
        assert_eq!(
            ExpressionParser::parse("event.1a.2.value").unwrap(),
            Expression::field_access(vec![
                "event".into(),
                "1a".into(),
                "2".into(),
                "value".into()
            ])
        );
    }

    #[test]
    fn quoted_strings_are_atomic_and_unicode_safe() {
        for (text, value) in [
            (r#""中国""#, "中国"),
            (
                r#"'high-risk/a+b (x) && y, in [z]'"#,
                "high-risk/a+b (x) && y, in [z]",
            ),
            (r#""a\n\t\"b\\c""#, "a\n\t\"b\\c"),
            (r#""\u4e2d\u56fd\uD83D\uDE00""#, "中国😀"),
            (r#"'it\'s'"#, "it's"),
        ] {
            assert_eq!(
                ExpressionParser::parse(text).unwrap(),
                Expression::literal(Value::String(value.into())),
                "{text}"
            );
        }
    }

    #[test]
    fn malformed_expressions_return_errors_without_panicking() {
        for text in [
            "",
            "中文 @",
            "\"中国",
            "'abc",
            r#""\uD800""#,
            r#""\uDC00""#,
            r#""\q""#,
            "1e-",
            "a = b",
            "a & b",
            "a ||",
            "(a",
            "a)",
            "a b",
            "[1,]",
            "f(1,)",
            "event..a",
        ] {
            assert!(ExpressionParser::parse(text).is_err(), "{text}");
        }
        assert!(
            ExpressionParser::parse(&format!("{}true{}", "(".repeat(129), ")".repeat(129)))
                .is_err()
        );
    }

    #[test]
    fn test_parse_number_literal() {
        let expr = ExpressionParser::parse("42").unwrap();
        assert_eq!(expr, Expression::literal(Value::Number(42.0)));

        let expr = ExpressionParser::parse("3.5").unwrap();
        assert_eq!(expr, Expression::literal(Value::Number(3.5)));
    }

    #[test]
    fn test_parse_string_literal() {
        let expr = ExpressionParser::parse(r#""hello world""#).unwrap();
        assert_eq!(
            expr,
            Expression::literal(Value::String("hello world".to_string()))
        );
    }

    #[test]
    fn test_parse_boolean_literal() {
        let expr = ExpressionParser::parse("true").unwrap();
        assert_eq!(expr, Expression::literal(Value::Bool(true)));

        let expr = ExpressionParser::parse("false").unwrap();
        assert_eq!(expr, Expression::literal(Value::Bool(false)));
    }

    #[test]
    fn test_parse_null_literal() {
        let expr = ExpressionParser::parse("null").unwrap();
        assert_eq!(expr, Expression::literal(Value::Null));
    }

    #[test]
    fn test_parse_field_access() {
        let expr = ExpressionParser::parse("user.age").unwrap();
        assert_eq!(
            expr,
            Expression::field_access(vec!["user".to_string(), "age".to_string()])
        );

        let expr = ExpressionParser::parse("event.device.id").unwrap();
        assert_eq!(
            expr,
            Expression::field_access(vec![
                "event".to_string(),
                "device".to_string(),
                "id".to_string()
            ])
        );
    }

    #[test]
    fn test_parse_binary_comparison() {
        let expr = ExpressionParser::parse("user.age > 18").unwrap();
        assert!(matches!(expr, Expression::Binary { .. }));

        let expr = ExpressionParser::parse("amount >= 1000").unwrap();
        assert!(matches!(expr, Expression::Binary { .. }));

        let expr = ExpressionParser::parse("status == \"active\"").unwrap();
        assert!(matches!(expr, Expression::Binary { .. }));
    }

    #[test]
    fn test_parse_binary_arithmetic() {
        let expr = ExpressionParser::parse("a + b").unwrap();
        assert!(matches!(expr, Expression::Binary { .. }));

        let expr = ExpressionParser::parse("x * y").unwrap();
        assert!(matches!(expr, Expression::Binary { .. }));
    }

    #[test]
    fn test_parse_logical_operators() {
        let expr = ExpressionParser::parse("a && b").unwrap();
        assert!(matches!(expr, Expression::Binary { .. }));

        let expr = ExpressionParser::parse("x || y").unwrap();
        assert!(matches!(expr, Expression::Binary { .. }));
    }

    #[test]
    fn test_parse_complex_expression() {
        // (user.age > 18) && (country == "US")
        let expr = ExpressionParser::parse(r#"user.age > 18 && country == "US""#).unwrap();
        assert!(matches!(expr, Expression::Binary { .. }));
    }

    #[test]
    fn test_parse_function_call() {
        let expr = ExpressionParser::parse("count(user.logins)").unwrap();

        if let Expression::FunctionCall { name, args } = expr {
            assert_eq!(name, "count");
            assert_eq!(args.len(), 1);
        } else {
            panic!("Expected function call");
        }
    }

    #[test]
    fn test_parse_function_with_multiple_args() {
        let expr = ExpressionParser::parse("sum(amounts, 100)").unwrap();

        if let Expression::FunctionCall { name, args } = expr {
            assert_eq!(name, "sum");
            assert_eq!(args.len(), 2);
        } else {
            panic!("Expected function call");
        }
    }

    #[test]
    fn test_parse_unary_not() {
        let expr = ExpressionParser::parse("!user.active").unwrap();
        assert!(matches!(expr, Expression::Unary { .. }));
    }

    #[test]
    fn test_parse_with_parentheses() {
        let expr = ExpressionParser::parse("(a + b) * c").unwrap();
        assert!(matches!(expr, Expression::Binary { .. }));
    }

    #[test]
    fn test_invalid_expression() {
        let result = ExpressionParser::parse("");
        assert!(result.is_err());

        let result = ExpressionParser::parse("@#$");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_array_literal() {
        // Empty array
        let expr = ExpressionParser::parse("[]").unwrap();
        assert_eq!(expr, Expression::literal(Value::Array(Vec::new())));

        // Array with strings
        let expr = ExpressionParser::parse(r#"["a", "b", "c"]"#).unwrap();
        assert_eq!(
            expr,
            Expression::literal(Value::Array(vec![
                Value::String("a".to_string()),
                Value::String("b".to_string()),
                Value::String("c".to_string()),
            ]))
        );

        // Array with numbers
        let expr = ExpressionParser::parse("[1, 2, 3]").unwrap();
        assert_eq!(
            expr,
            Expression::literal(Value::Array(vec![
                Value::Number(1.0),
                Value::Number(2.0),
                Value::Number(3.0),
            ]))
        );
    }

    #[test]
    fn test_parse_in_operator_with_array() {
        let expr = ExpressionParser::parse(r#"event.country in ["RU", "CN", "NK"]"#).unwrap();
        if let Expression::Binary { op, right, .. } = &expr {
            assert_eq!(*op, Operator::In);
            assert_eq!(
                *right.clone(),
                Expression::literal(Value::Array(vec![
                    Value::String("RU".to_string()),
                    Value::String("CN".to_string()),
                    Value::String("NK".to_string()),
                ]))
            );
        } else {
            panic!("Expected binary expression");
        }
    }
}

// Each child has already been bounded, so rejection never drops an unbounded tree.
fn check_ast_depth(expression: &Expression) -> Result<()> {
    let mut pending = vec![(expression, 1usize)];
    while let Some((expr, depth)) = pending.pop() {
        if depth > 128 {
            return Err(invalid("Expression AST depth exceeds 128"));
        }
        match expr {
            Expression::Binary { left, right, .. } => {
                pending.extend([(left.as_ref(), depth + 1), (right.as_ref(), depth + 1)])
            }
            Expression::Unary { operand, .. } => pending.push((operand, depth + 1)),
            Expression::FunctionCall { args, .. }
            | Expression::LogicalGroup {
                conditions: args, ..
            } => pending.extend(args.iter().map(|arg| (arg, depth + 1))),
            Expression::Ternary {
                condition,
                true_expr,
                false_expr,
            } => pending.extend([
                (condition.as_ref(), depth + 1),
                (true_expr.as_ref(), depth + 1),
                (false_expr.as_ref(), depth + 1),
            ]),
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod resource_limit_tests {
    use super::*;
    #[test]
    fn flat_chains_are_bounded_before_recursive_consumers() {
        for (operand, op) in [("true", " && "), ("false", " || "), ("1", " + ")] {
            assert!(ExpressionParser::parse(&vec![operand; 128].join(op)).is_ok());
            assert!(ExpressionParser::parse(&vec![operand; 1024].join(op))
                .unwrap_err()
                .to_string()
                .contains("depth"));
        }
    }
}
