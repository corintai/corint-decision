//! Type checker
//!
//! Performs type inference and validation on expressions.

use crate::error::{CompileError, Result};
use corint_core::ast::{Expression, Operator, UnaryOperator};
use corint_core::Value;
use std::collections::HashMap;

/// Type information for expressions
#[derive(Debug, Clone, PartialEq)]
pub enum TypeInfo {
    Number,
    String,
    Boolean,
    Array(Box<TypeInfo>),
    Object,
    Any,
    Unknown,
}

impl TypeInfo {
    /// Check if this type is compatible with another type
    pub fn is_compatible_with(&self, other: &TypeInfo) -> bool {
        match (self, other) {
            (TypeInfo::Any, _) | (_, TypeInfo::Any) => true,
            (TypeInfo::Unknown, _) | (_, TypeInfo::Unknown) => true,
            (TypeInfo::Number, TypeInfo::Number) => true,
            (TypeInfo::String, TypeInfo::String) => true,
            (TypeInfo::Boolean, TypeInfo::Boolean) => true,
            (TypeInfo::Object, TypeInfo::Object) => true,
            (TypeInfo::Array(a), TypeInfo::Array(b)) => a.is_compatible_with(b),
            _ => false,
        }
    }

    /// Check if this type can be used in a numeric operation
    pub fn is_numeric(&self) -> bool {
        matches!(self, TypeInfo::Number | TypeInfo::Any | TypeInfo::Unknown)
    }

    /// Check if this type can be used in a boolean operation
    pub fn is_boolean(&self) -> bool {
        matches!(self, TypeInfo::Boolean | TypeInfo::Any | TypeInfo::Unknown)
    }

    /// Check if this type can be compared
    pub fn is_comparable(&self) -> bool {
        matches!(
            self,
            TypeInfo::Number
                | TypeInfo::String
                | TypeInfo::Boolean
                | TypeInfo::Any
                | TypeInfo::Unknown
        )
    }
}

/// Type checker
pub struct TypeChecker {
    /// Type information for known variables
    variable_types: HashMap<String, TypeInfo>,
    /// Type information for known fields
    field_types: HashMap<String, TypeInfo>,
}

impl TypeChecker {
    /// Create a new type checker
    pub fn new() -> Self {
        Self {
            variable_types: HashMap::new(),
            field_types: HashMap::new(),
        }
    }

    /// Register a variable with its type
    pub fn register_variable(&mut self, name: String, type_info: TypeInfo) {
        self.variable_types.insert(name, type_info);
    }

    /// Register a field with its type
    pub fn register_field(&mut self, path: String, type_info: TypeInfo) {
        self.field_types.insert(path, type_info);
    }

    /// Get the type of a variable
    pub fn get_variable_type(&self, name: &str) -> Option<&TypeInfo> {
        self.variable_types.get(name)
    }

    /// Get the type of a field
    pub fn get_field_type(&self, path: &str) -> Option<&TypeInfo> {
        self.field_types.get(path)
    }

    /// Infer and validate the type of an expression
    pub fn check_expression(&self, expr: &Expression) -> Result<TypeInfo> {
        match expr {
            Expression::Literal(value) => Ok(self.infer_literal_type(value)),

            Expression::FieldAccess(path) => {
                let path_str = path.join(".");
                // If we have type information for this field, use it
                if let Some(type_info) = self.field_types.get(&path_str) {
                    Ok(type_info.clone())
                } else {
                    // Otherwise, assume it's unknown
                    Ok(TypeInfo::Unknown)
                }
            }

            Expression::Binary { left, op, right } => {
                let left_type = self.check_expression(left)?;
                let right_type = self.check_expression(right)?;
                self.check_binary_operation(&left_type, op, &right_type)
            }

            Expression::Unary { op, operand } => {
                let operand_type = self.check_expression(operand)?;
                self.check_unary_operation(op, &operand_type)
            }

            Expression::FunctionCall { name: _, args } => {
                // Check all argument types
                for arg in args {
                    self.check_expression(arg)?;
                }
                // For now, assume functions return unknown type
                // In a full implementation, we'd have a function registry
                Ok(TypeInfo::Unknown)
            }

            Expression::Ternary {
                condition,
                true_expr,
                false_expr,
            } => {
                let condition_type = self.check_expression(condition)?;
                let true_type = self.check_expression(true_expr)?;
                let false_type = self.check_expression(false_expr)?;

                // Condition must be boolean
                if !condition_type.is_boolean() && !matches!(condition_type, TypeInfo::Unknown) {
                    return Err(CompileError::TypeError(
                        "Ternary condition must be boolean".to_string(),
                    ));
                }

                // Both branches should have compatible types
                if true_type.is_compatible_with(&false_type) {
                    Ok(true_type)
                } else if false_type.is_compatible_with(&true_type) {
                    Ok(false_type)
                } else {
                    Ok(TypeInfo::Any)
                }
            }

            Expression::LogicalGroup { conditions, .. } => {
                // All conditions must be boolean
                for condition in conditions {
                    let condition_type = self.check_expression(condition)?;
                    if !condition_type.is_boolean() && !matches!(condition_type, TypeInfo::Unknown)
                    {
                        return Err(CompileError::TypeError(
                            "Logical group conditions must be boolean".to_string(),
                        ));
                    }
                }
                // Result is always boolean
                Ok(TypeInfo::Boolean)
            }

            Expression::ListReference { .. } => {
                // List reference type is unknown at compile time
                // The actual list will be resolved at runtime
                Ok(TypeInfo::Unknown)
            }
        }
    }

    /// Infer the type of a literal value
    #[allow(clippy::only_used_in_recursion)]
    fn infer_literal_type(&self, value: &Value) -> TypeInfo {
        match value {
            Value::Number(_) => TypeInfo::Number,
            Value::String(_) => TypeInfo::String,
            Value::Bool(_) => TypeInfo::Boolean,
            Value::Array(arr) => {
                if arr.is_empty() {
                    TypeInfo::Array(Box::new(TypeInfo::Unknown))
                } else {
                    let elem_type = self.infer_literal_type(&arr[0]);
                    TypeInfo::Array(Box::new(elem_type))
                }
            }
            Value::Object(_) => TypeInfo::Object,
            Value::Null => TypeInfo::Any,
        }
    }

    /// Check if a binary operation is valid for the given types
    fn check_binary_operation(
        &self,
        left: &TypeInfo,
        op: &Operator,
        right: &TypeInfo,
    ) -> Result<TypeInfo> {
        match op {
            // Arithmetic operators require numeric operands
            Operator::Add | Operator::Sub | Operator::Mul | Operator::Div | Operator::Mod => {
                if !left.is_numeric() && !matches!(left, TypeInfo::Unknown) {
                    return Err(CompileError::TypeError(format!(
                        "Left operand of {:?} must be numeric",
                        op
                    )));
                }
                if !right.is_numeric() && !matches!(right, TypeInfo::Unknown) {
                    return Err(CompileError::TypeError(format!(
                        "Right operand of {:?} must be numeric",
                        op
                    )));
                }
                Ok(TypeInfo::Number)
            }

            // Comparison operators require comparable operands
            Operator::Eq
            | Operator::Ne
            | Operator::Gt
            | Operator::Ge
            | Operator::Lt
            | Operator::Le => {
                if !left.is_comparable() && !matches!(left, TypeInfo::Unknown) {
                    return Err(CompileError::TypeError(
                        "Left operand must be comparable".to_string(),
                    ));
                }
                if !right.is_comparable() && !matches!(right, TypeInfo::Unknown) {
                    return Err(CompileError::TypeError(
                        "Right operand must be comparable".to_string(),
                    ));
                }
                // Type compatibility check
                if !left.is_compatible_with(right)
                    && !right.is_compatible_with(left)
                    && !matches!(left, TypeInfo::Unknown)
                    && !matches!(right, TypeInfo::Unknown)
                {
                    return Err(CompileError::TypeError(
                        "Operands must have compatible types".to_string(),
                    ));
                }
                Ok(TypeInfo::Boolean)
            }

            // Logical operators require boolean operands
            Operator::And | Operator::Or => {
                if !left.is_boolean() && !matches!(left, TypeInfo::Unknown) {
                    return Err(CompileError::TypeError(format!(
                        "Left operand of {:?} must be boolean",
                        op
                    )));
                }
                if !right.is_boolean() && !matches!(right, TypeInfo::Unknown) {
                    return Err(CompileError::TypeError(format!(
                        "Right operand of {:?} must be boolean",
                        op
                    )));
                }
                Ok(TypeInfo::Boolean)
            }

            // In operator: left can be any, right should be array
            Operator::In => {
                if !matches!(
                    right,
                    TypeInfo::Array(_) | TypeInfo::Unknown | TypeInfo::Any
                ) {
                    return Err(CompileError::TypeError(
                        "Right operand of 'in' must be an array".to_string(),
                    ));
                }
                Ok(TypeInfo::Boolean)
            }

            // NotIn operator: similar to In
            Operator::NotIn => {
                if !matches!(
                    right,
                    TypeInfo::Array(_) | TypeInfo::Unknown | TypeInfo::Any
                ) {
                    return Err(CompileError::TypeError(
                        "Right operand of 'not in' must be an array".to_string(),
                    ));
                }
                Ok(TypeInfo::Boolean)
            }

            // String operators: require string operands
            Operator::Contains | Operator::StartsWith | Operator::EndsWith | Operator::Regex => {
                if !matches!(left, TypeInfo::String | TypeInfo::Unknown | TypeInfo::Any) {
                    return Err(CompileError::TypeError(
                        "Left operand must be string".to_string(),
                    ));
                }
                if !matches!(right, TypeInfo::String | TypeInfo::Unknown | TypeInfo::Any) {
                    return Err(CompileError::TypeError(
                        "Right operand must be string".to_string(),
                    ));
                }
                Ok(TypeInfo::Boolean)
            }

            // List membership operators: left can be any, right should be ListReference (Unknown)
            Operator::InList | Operator::NotInList => {
                // Left operand can be any type - it's the value to check
                // Right operand should be a ListReference, which has Unknown type
                // We allow Unknown/Any for the right side since ListReference resolves to Unknown
                Ok(TypeInfo::Boolean)
            }
        }
    }

    /// Check if a unary operation is valid for the given type
    fn check_unary_operation(&self, op: &UnaryOperator, operand: &TypeInfo) -> Result<TypeInfo> {
        match op {
            UnaryOperator::Not => {
                if !operand.is_boolean() && !matches!(operand, TypeInfo::Unknown) {
                    return Err(CompileError::TypeError(
                        "Operand of 'not' must be boolean".to_string(),
                    ));
                }
                Ok(TypeInfo::Boolean)
            }
            UnaryOperator::Negate => {
                if !operand.is_numeric() && !matches!(operand, TypeInfo::Unknown) {
                    return Err(CompileError::TypeError(
                        "Operand of negation must be numeric".to_string(),
                    ));
                }
                Ok(TypeInfo::Number)
            }
        }
    }
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_compatibility() {
        assert!(TypeInfo::Number.is_compatible_with(&TypeInfo::Number));
        assert!(TypeInfo::String.is_compatible_with(&TypeInfo::String));
        assert!(TypeInfo::Boolean.is_compatible_with(&TypeInfo::Boolean));
        assert!(!TypeInfo::Number.is_compatible_with(&TypeInfo::String));

        // Any is compatible with everything
        assert!(TypeInfo::Any.is_compatible_with(&TypeInfo::Number));
        assert!(TypeInfo::Number.is_compatible_with(&TypeInfo::Any));

        // Unknown is compatible with everything
        assert!(TypeInfo::Unknown.is_compatible_with(&TypeInfo::Number));
        assert!(TypeInfo::Number.is_compatible_with(&TypeInfo::Unknown));
    }

    #[test]
    fn test_infer_literal_types() {
        let checker = TypeChecker::new();

        assert_eq!(
            checker.infer_literal_type(&Value::Number(42.0)),
            TypeInfo::Number
        );
        assert_eq!(
            checker.infer_literal_type(&Value::String("test".to_string())),
            TypeInfo::String
        );
        assert_eq!(
            checker.infer_literal_type(&Value::Bool(true)),
            TypeInfo::Boolean
        );
        assert_eq!(checker.infer_literal_type(&Value::Null), TypeInfo::Any);
    }

    #[test]
    fn test_check_literal_expression() {
        let checker = TypeChecker::new();
        let expr = Expression::literal(Value::Number(42.0));

        let type_info = checker.check_expression(&expr).unwrap();
        assert_eq!(type_info, TypeInfo::Number);
    }

    #[test]
    fn test_check_field_access() {
        let mut checker = TypeChecker::new();
        checker.register_field("user.age".to_string(), TypeInfo::Number);

        let expr = Expression::field_access(vec!["user".to_string(), "age".to_string()]);
        let type_info = checker.check_expression(&expr).unwrap();
        assert_eq!(type_info, TypeInfo::Number);
    }

    #[test]
    fn test_check_arithmetic_operation() {
        let checker = TypeChecker::new();
        let expr = Expression::binary(
            Expression::literal(Value::Number(10.0)),
            Operator::Add,
            Expression::literal(Value::Number(20.0)),
        );

        let type_info = checker.check_expression(&expr).unwrap();
        assert_eq!(type_info, TypeInfo::Number);
    }

    #[test]
    fn test_check_arithmetic_type_error() {
        let checker = TypeChecker::new();
        let expr = Expression::binary(
            Expression::literal(Value::String("test".to_string())),
            Operator::Add,
            Expression::literal(Value::Number(20.0)),
        );

        let result = checker.check_expression(&expr);
        assert!(result.is_err());
    }

    #[test]
    fn test_check_comparison_operation() {
        let checker = TypeChecker::new();
        let expr = Expression::binary(
            Expression::literal(Value::Number(10.0)),
            Operator::Gt,
            Expression::literal(Value::Number(5.0)),
        );

        let type_info = checker.check_expression(&expr).unwrap();
        assert_eq!(type_info, TypeInfo::Boolean);
    }

    #[test]
    fn test_check_comparison_type_mismatch() {
        let checker = TypeChecker::new();
        let expr = Expression::binary(
            Expression::literal(Value::Number(10.0)),
            Operator::Gt,
            Expression::literal(Value::String("test".to_string())),
        );

        let result = checker.check_expression(&expr);
        assert!(result.is_err());
    }

    #[test]
    fn test_check_logical_operation() {
        let checker = TypeChecker::new();
        let expr = Expression::binary(
            Expression::literal(Value::Bool(true)),
            Operator::And,
            Expression::literal(Value::Bool(false)),
        );

        let type_info = checker.check_expression(&expr).unwrap();
        assert_eq!(type_info, TypeInfo::Boolean);
    }

    #[test]
    fn test_check_logical_type_error() {
        let checker = TypeChecker::new();
        let expr = Expression::binary(
            Expression::literal(Value::Number(10.0)),
            Operator::And,
            Expression::literal(Value::Bool(false)),
        );

        let result = checker.check_expression(&expr);
        assert!(result.is_err());
    }

    #[test]
    fn test_check_unary_not() {
        let checker = TypeChecker::new();
        let expr = Expression::unary(UnaryOperator::Not, Expression::literal(Value::Bool(true)));

        let type_info = checker.check_expression(&expr).unwrap();
        assert_eq!(type_info, TypeInfo::Boolean);
    }

    #[test]
    fn test_check_unary_not_type_error() {
        let checker = TypeChecker::new();
        let expr = Expression::unary(UnaryOperator::Not, Expression::literal(Value::Number(42.0)));

        let result = checker.check_expression(&expr);
        assert!(result.is_err());
    }

    #[test]
    fn test_check_unary_neg() {
        let checker = TypeChecker::new();
        let expr = Expression::unary(
            UnaryOperator::Negate,
            Expression::literal(Value::Number(42.0)),
        );

        let type_info = checker.check_expression(&expr).unwrap();
        assert_eq!(type_info, TypeInfo::Number);
    }

    #[test]
    fn test_check_ternary_expression() {
        let checker = TypeChecker::new();
        let expr = Expression::ternary(
            Expression::literal(Value::Bool(true)),
            Expression::literal(Value::Number(10.0)),
            Expression::literal(Value::Number(20.0)),
        );

        let type_info = checker.check_expression(&expr).unwrap();
        assert_eq!(type_info, TypeInfo::Number);
    }

    #[test]
    fn test_check_ternary_condition_type_error() {
        let checker = TypeChecker::new();
        let expr = Expression::ternary(
            Expression::literal(Value::Number(42.0)),
            Expression::literal(Value::Number(10.0)),
            Expression::literal(Value::Number(20.0)),
        );

        let result = checker.check_expression(&expr);
        assert!(result.is_err());
    }

    #[test]
    fn test_check_in_operator() {
        let checker = TypeChecker::new();
        let expr = Expression::binary(
            Expression::literal(Value::String("test".to_string())),
            Operator::In,
            Expression::literal(Value::Array(vec![Value::String("test".to_string())])),
        );

        let type_info = checker.check_expression(&expr).unwrap();
        assert_eq!(type_info, TypeInfo::Boolean);
    }

    #[test]
    fn test_variable_registration() {
        let mut checker = TypeChecker::new();

        assert!(checker.get_variable_type("x").is_none());

        checker.register_variable("x".to_string(), TypeInfo::Number);

        assert_eq!(checker.get_variable_type("x"), Some(&TypeInfo::Number));
    }
}
