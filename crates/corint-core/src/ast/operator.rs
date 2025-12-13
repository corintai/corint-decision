//! Operators for CORINT expressions

use serde::{Deserialize, Serialize};

/// Binary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Operator {
    // Comparison operators
    /// Equal (==)
    Eq,
    /// Not equal (!=)
    Ne,
    /// Greater than (>)
    Gt,
    /// Greater than or equal (>=)
    Ge,
    /// Less than (<)
    Lt,
    /// Less than or equal (<=)
    Le,

    // Arithmetic operators
    /// Addition (+)
    Add,
    /// Subtraction (-)
    Sub,
    /// Multiplication (*)
    Mul,
    /// Division (/)
    Div,
    /// Modulo (%)
    Mod,

    // Logical operators
    /// Logical AND (&&)
    And,
    /// Logical OR (||)
    Or,

    // String operators
    /// Contains
    Contains,
    /// Starts with
    StartsWith,
    /// Ends with
    EndsWith,
    /// Regex match
    Regex,

    // Membership operators
    /// In (element in array/list)
    In,
    /// Not in
    NotIn,
}

impl Operator {
    /// Returns true if this is a comparison operator
    pub fn is_comparison(&self) -> bool {
        matches!(
            self,
            Operator::Eq | Operator::Ne | Operator::Gt | Operator::Ge | Operator::Lt | Operator::Le
        )
    }

    /// Returns true if this is an arithmetic operator
    pub fn is_arithmetic(&self) -> bool {
        matches!(
            self,
            Operator::Add | Operator::Sub | Operator::Mul | Operator::Div | Operator::Mod
        )
    }

    /// Returns true if this is a logical operator
    pub fn is_logical(&self) -> bool {
        matches!(self, Operator::And | Operator::Or)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operator_equality() {
        assert_eq!(Operator::Eq, Operator::Eq);
        assert_ne!(Operator::Eq, Operator::Ne);
    }

    #[test]
    fn test_operator_is_comparison() {
        assert!(Operator::Eq.is_comparison());
        assert!(Operator::Gt.is_comparison());
        assert!(Operator::Lt.is_comparison());
        assert!(!Operator::Add.is_comparison());
        assert!(!Operator::And.is_comparison());
    }

    #[test]
    fn test_operator_is_arithmetic() {
        assert!(Operator::Add.is_arithmetic());
        assert!(Operator::Mul.is_arithmetic());
        assert!(!Operator::Eq.is_arithmetic());
        assert!(!Operator::And.is_arithmetic());
    }

    #[test]
    fn test_operator_is_logical() {
        assert!(Operator::And.is_logical());
        assert!(Operator::Or.is_logical());
        assert!(!Operator::Eq.is_logical());
        assert!(!Operator::Add.is_logical());
    }

    #[test]
    fn test_operator_clone() {
        let op = Operator::Eq;
        let cloned = op;
        assert_eq!(op, cloned);
    }
}
