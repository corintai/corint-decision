//! Intermediate Representation (IR) for CORINT
//!
//! The IR is a lower-level representation optimized for execution.
//! It serves as the target of compilation from AST.

pub mod instruction;
pub mod program;

pub use instruction::{FeatureType, Instruction, TimeWindow};
pub use program::{Program, ProgramMetadata};
