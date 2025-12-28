//! Expression parser
//!
//! Parses string expressions into Expression AST nodes.
//!
//! Supported syntax:
//! - Field access: `user.age`, `event.device.id`
//! - Literals: `42`, `3.14`, `"string"`, `true`, `false`, `null`
//! - Binary operators: `>`, `<`, `>=`, `<=`, `==`, `!=`, `+`, `-`, `*`, `/`, `&&`, `||`
//! - Unary operators: `!`, `-`
//! - Function calls: `count(user.logins)`, `sum(amounts, last_7d)`
//! - Parentheses for grouping: `(a + b) * c`

use crate::error::{ParseError, Result};
use corint_core::ast::{Expression, Operator, UnaryOperator};
use corint_core::Value;

/// Expression parser
pub struct ExpressionParser;

impl ExpressionParser {
    /// Parse an expression from a string
    pub fn parse(input: &str) -> Result<Expression> {
        let input = input.trim();

        if input.is_empty() {
            return Err(ParseError::InvalidExpression(
                "Empty expression".to_string(),
            ));
        }

        Self::parse_expression(input)
    }

    /// Parse a complete expression (handles binary operators with precedence)
    fn parse_expression(input: &str) -> Result<Expression> {
        // Try to parse as binary expression with logical operators (lowest precedence)
        if let Some((left, op, right)) = Self::split_by_operator(input, &["||", "&&"]) {
            let op = Self::parse_operator(op)?;
            return Ok(Expression::binary(
                Self::parse_expression(left)?,
                op,
                Self::parse_expression(right)?,
            ));
        }

        // Try to parse as binary expression with keyword operators (contains, in, not in, etc.)
        // Note: "not in" must come before "in" to match correctly
        if let Some((left, op, right)) = Self::split_by_keyword_operator(
            input,
            &[
                "not in",     // Must be before "in"
                "contains",
                "in",
                "starts_with",
                "ends_with",
                "regex",
            ],
        ) {
            // Special handling for "in list.xxx" and "not in list.xxx"
            if (op == "in" || op == "not in") && right.trim().starts_with("list.") {
                let list_id = right.trim().strip_prefix("list.").unwrap().to_string();
                let operator = if op == "in" {
                    Operator::InList
                } else {
                    Operator::NotInList
                };
                return Ok(Expression::binary(
                    Self::parse_expression(left)?,
                    operator,
                    Expression::ListReference { list_id },
                ));
            }

            let op = Self::parse_operator(op)?;
            return Ok(Expression::binary(
                Self::parse_expression(left)?,
                op,
                Self::parse_expression(right)?,
            ));
        }

        // Try to parse as binary expression with comparison operators
        if let Some((left, op, right)) =
            Self::split_by_operator(input, &["==", "!=", "<=", ">=", "<", ">"])
        {
            let op = Self::parse_operator(op)?;
            return Ok(Expression::binary(
                Self::parse_expression(left)?,
                op,
                Self::parse_expression(right)?,
            ));
        }

        // Try to parse as binary expression with additive operators
        if let Some((left, op, right)) = Self::split_by_operator(input, &["+", "-"]) {
            let op = Self::parse_operator(op)?;
            return Ok(Expression::binary(
                Self::parse_expression(left)?,
                op,
                Self::parse_expression(right)?,
            ));
        }

        // Try to parse as binary expression with multiplicative operators
        if let Some((left, op, right)) = Self::split_by_operator(input, &["*", "/", "%"]) {
            let op = Self::parse_operator(op)?;
            return Ok(Expression::binary(
                Self::parse_expression(left)?,
                op,
                Self::parse_expression(right)?,
            ));
        }

        // Parse primary expression (literals, field access, function calls, parentheses)
        Self::parse_primary(input)
    }

    /// Parse a primary expression
    fn parse_primary(input: &str) -> Result<Expression> {
        let input = input.trim();

        // Check for unary operators
        if let Some(stripped) = input.strip_prefix('!') {
            return Ok(Expression::Unary {
                op: UnaryOperator::Not,
                operand: Box::new(Self::parse_primary(stripped.trim())?),
            });
        }

        if input.starts_with('-') && !input[1..].trim().starts_with(|c: char| c.is_ascii_digit()) {
            return Ok(Expression::Unary {
                op: UnaryOperator::Negate,
                operand: Box::new(Self::parse_primary(input[1..].trim())?),
            });
        }

        // Check for parentheses
        if input.starts_with('(') && input.ends_with(')') {
            return Self::parse_expression(&input[1..input.len() - 1]);
        }

        // Check for string literals
        if input.starts_with('"') && input.ends_with('"') {
            let s = &input[1..input.len() - 1];
            return Ok(Expression::literal(Value::String(s.to_string())));
        }

        // Check for boolean literals
        if input == "true" {
            return Ok(Expression::literal(Value::Bool(true)));
        }
        if input == "false" {
            return Ok(Expression::literal(Value::Bool(false)));
        }
        if input == "null" {
            return Ok(Expression::literal(Value::Null));
        }

        // Check for number literals
        if let Ok(num) = input.parse::<f64>() {
            return Ok(Expression::literal(Value::Number(num)));
        }

        // Check for array literals like ["a", "b", "c"]
        if input.starts_with('[') && input.ends_with(']') {
            let inner = &input[1..input.len() - 1].trim();
            if inner.is_empty() {
                return Ok(Expression::literal(Value::Array(Vec::new())));
            }
            let elements = Self::parse_array_elements(inner)?;
            return Ok(Expression::literal(Value::Array(elements)));
        }

        // Check for function calls
        if let Some(paren_pos) = input.find('(') {
            if input.ends_with(')') {
                let func_name = input[..paren_pos].trim();
                let args_str = &input[paren_pos + 1..input.len() - 1];

                let args = if args_str.trim().is_empty() {
                    Vec::new()
                } else {
                    Self::parse_function_args(args_str)?
                };

                return Ok(Expression::function_call(func_name.to_string(), args));
            }
        }

        // Check for result access: result.field or result.ruleset_id.field
        // Support both "result." and "results." forms
        let (is_result_access, rest_str) = if input.starts_with("results.") {
            (true, &input[8..]) // Skip "results."
        } else if input.starts_with("result.") {
            (true, &input[7..]) // Skip "result."
        } else {
            (false, "")
        };

        if is_result_access {
            let parts: Vec<&str> = rest_str.split('.').collect();

            if parts.is_empty() {
                return Err(ParseError::InvalidExpression(
                    "result/results requires a field name".to_string(),
                ));
            }

            if parts.len() == 1 {
                // result.field - access last ruleset's result
                return Ok(Expression::ResultAccess {
                    ruleset_id: None,
                    field: parts[0].trim().to_string(),
                });
            } else {
                // result.ruleset_id.field - access specific ruleset's result
                let ruleset_id = parts[0].trim().to_string();
                let field = parts[1..].join(".");
                return Ok(Expression::ResultAccess {
                    ruleset_id: Some(ruleset_id),
                    field: field.trim().to_string(),
                });
            }
        }

        // Must be field access
        if input.contains('.') {
            let parts: Vec<String> = input.split('.').map(|s| s.trim().to_string()).collect();
            return Ok(Expression::field_access(parts));
        }

        // Single identifier is also field access
        if input.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Ok(Expression::field_access(vec![input.to_string()]));
        }

        Err(ParseError::InvalidExpression(format!(
            "Cannot parse: {}",
            input
        )))
    }

    /// Split input by binary operator (respecting parentheses and brackets)
    fn split_by_operator<'a>(
        input: &'a str,
        operators: &[&str],
    ) -> Option<(&'a str, &'a str, &'a str)> {
        let mut paren_depth = 0;
        let mut bracket_depth = 0;
        let bytes = input.as_bytes();

        // Scan from right to left to handle left-to-right associativity
        for i in (0..input.len()).rev() {
            let c = bytes[i] as char;

            if c == ')' {
                paren_depth += 1;
            } else if c == '(' {
                paren_depth -= 1;
            } else if c == ']' {
                bracket_depth += 1;
            } else if c == '[' {
                bracket_depth -= 1;
            }

            if paren_depth == 0 && bracket_depth == 0 {
                for &op in operators {
                    if i + op.len() <= input.len() && &input[i..i + op.len()] == op {
                        // Make sure it's not part of another operator
                        let is_valid = (i == 0 || !Self::is_operator_char(bytes[i - 1] as char))
                            && (i + op.len() >= input.len()
                                || !Self::is_operator_char(bytes[i + op.len()] as char));

                        if is_valid {
                            return Some((
                                input[..i].trim(),
                                &input[i..i + op.len()],
                                input[i + op.len()..].trim(),
                            ));
                        }
                    }
                }
            }
        }

        None
    }

    /// Split input by keyword operator (respecting parentheses, brackets, and word boundaries)
    fn split_by_keyword_operator<'a>(
        input: &'a str,
        operators: &[&str],
    ) -> Option<(&'a str, &'a str, &'a str)> {
        let mut paren_depth = 0;
        let mut bracket_depth = 0;
        let bytes = input.as_bytes();

        // Scan from right to left to handle left-to-right associativity
        for i in (0..input.len()).rev() {
            let c = bytes[i] as char;

            if c == ')' {
                paren_depth += 1;
            } else if c == '(' {
                paren_depth -= 1;
            } else if c == ']' {
                bracket_depth += 1;
            } else if c == '[' {
                bracket_depth -= 1;
            }

            if paren_depth == 0 && bracket_depth == 0 {
                for &op in operators {
                    if i + op.len() <= input.len() && &input[i..i + op.len()] == op {
                        // For keyword operators, check word boundaries
                        let has_space_before = i == 0 || bytes[i - 1].is_ascii_whitespace();
                        let has_space_after = i + op.len() >= input.len()
                            || bytes[i + op.len()].is_ascii_whitespace()
                            || bytes[i + op.len()] == b'['; // Allow array literal after "in"

                        if has_space_before && has_space_after {
                            // Special check: if we matched "in", make sure it's not part of "not in"
                            // "not in" is 6 chars, so check if position i-4 starts with "not "
                            if op == "in" && i >= 4 {
                                let potential_not_in_start = i - 4;
                                if &input[potential_not_in_start..i] == "not " {
                                    // This "in" is part of "not in", skip it
                                    continue;
                                }
                            }

                            return Some((
                                input[..i].trim(),
                                &input[i..i + op.len()],
                                input[i + op.len()..].trim(),
                            ));
                        }
                    }
                }
            }
        }

        None
    }

    /// Check if a character is part of an operator
    fn is_operator_char(c: char) -> bool {
        matches!(
            c,
            '=' | '!' | '<' | '>' | '&' | '|' | '+' | '-' | '*' | '/' | '%'
        )
    }

    /// Parse function arguments
    fn parse_function_args(args_str: &str) -> Result<Vec<Expression>> {
        if args_str.trim().is_empty() {
            return Ok(Vec::new());
        }

        let mut args = Vec::new();
        let mut current_arg = String::new();
        let mut paren_depth = 0;
        let mut in_string = false;

        for c in args_str.chars() {
            match c {
                '"' => in_string = !in_string,
                '(' if !in_string => paren_depth += 1,
                ')' if !in_string => paren_depth -= 1,
                ',' if !in_string && paren_depth == 0 => {
                    args.push(Self::parse_expression(current_arg.trim())?);
                    current_arg.clear();
                    continue;
                }
                _ => {}
            }
            current_arg.push(c);
        }

        if !current_arg.trim().is_empty() {
            args.push(Self::parse_expression(current_arg.trim())?);
        }

        Ok(args)
    }

    /// Parse array elements (e.g., "a", "b", "c" or 1, 2, 3)
    fn parse_array_elements(elements_str: &str) -> Result<Vec<Value>> {
        if elements_str.trim().is_empty() {
            return Ok(Vec::new());
        }

        let mut elements = Vec::new();
        let mut current = String::new();
        let mut in_string = false;
        let mut bracket_depth = 0;

        for c in elements_str.chars() {
            match c {
                '"' => {
                    in_string = !in_string;
                    current.push(c);
                }
                '[' if !in_string => {
                    bracket_depth += 1;
                    current.push(c);
                }
                ']' if !in_string => {
                    bracket_depth -= 1;
                    current.push(c);
                }
                ',' if !in_string && bracket_depth == 0 => {
                    let value = Self::parse_value_literal(current.trim())?;
                    elements.push(value);
                    current.clear();
                }
                _ => {
                    current.push(c);
                }
            }
        }

        if !current.trim().is_empty() {
            let value = Self::parse_value_literal(current.trim())?;
            elements.push(value);
        }

        Ok(elements)
    }

    /// Parse a value literal (string, number, boolean, null)
    fn parse_value_literal(input: &str) -> Result<Value> {
        let input = input.trim();

        // String literal
        if input.starts_with('"') && input.ends_with('"') && input.len() >= 2 {
            return Ok(Value::String(input[1..input.len() - 1].to_string()));
        }

        // Boolean literals
        if input == "true" {
            return Ok(Value::Bool(true));
        }
        if input == "false" {
            return Ok(Value::Bool(false));
        }

        // Null literal
        if input == "null" {
            return Ok(Value::Null);
        }

        // Number literal
        if let Ok(num) = input.parse::<f64>() {
            return Ok(Value::Number(num));
        }

        Err(ParseError::InvalidExpression(format!(
            "Invalid array element: {}",
            input
        )))
    }

    /// Parse an operator string
    fn parse_operator(op: &str) -> Result<Operator> {
        match op {
            "==" => Ok(Operator::Eq),
            "!=" => Ok(Operator::Ne),
            "<" => Ok(Operator::Lt),
            ">" => Ok(Operator::Gt),
            "<=" => Ok(Operator::Le),
            ">=" => Ok(Operator::Ge),
            "+" => Ok(Operator::Add),
            "-" => Ok(Operator::Sub),
            "*" => Ok(Operator::Mul),
            "/" => Ok(Operator::Div),
            "%" => Ok(Operator::Mod),
            "&&" => Ok(Operator::And),
            "||" => Ok(Operator::Or),
            "contains" => Ok(Operator::Contains),
            "starts_with" => Ok(Operator::StartsWith),
            "ends_with" => Ok(Operator::EndsWith),
            "regex" => Ok(Operator::Regex),
            "in" => Ok(Operator::In),
            "not in" => Ok(Operator::NotIn),
            "not_in" => Ok(Operator::NotIn), // Keep underscore version for compatibility
            _ => Err(ParseError::InvalidOperator(op.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
