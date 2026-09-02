//! Operator execution modules
//!
//! This module contains the implementation of operator evaluation for the IR executor.

mod binary;
mod comparison;
mod unary;

pub(crate) use binary::execute_binary_op;
pub(crate) use comparison::execute_compare;
pub(crate) use unary::execute_unary_op;
