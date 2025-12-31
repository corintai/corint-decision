//! Integration tests for Pipeline DAG compilation
//!
//! Tests the complete flow from Pipeline AST to IR Program with various scenarios

use corint_compiler::codegen::pipeline_codegen::PipelineCompiler;
use corint_core::ast::pipeline::{Pipeline, PipelineStep, Route, StepDetails, StepNext};
use corint_core::ast::rule::{Condition, ConditionGroup};
use corint_core::ast::{Expression, Operator, WhenBlock};
use corint_core::ir::instruction::Instruction;
use corint_core::Value;

/// Helper to create a simple step
fn create_step(id: &str, step_type: &str) -> PipelineStep {
    PipelineStep {
        id: id.to_string(),
        name: format!("Step {}", id),
        step_type: step_type.to_string(),
        routes: None,
        default: None,
        next: None,
        when: None,
        details: StepDetails::Unknown {},
    }
}

/// Helper to create a ruleset step
fn create_ruleset_step(id: &str, ruleset: &str, next: Option<&str>) -> PipelineStep {
    PipelineStep {
        id: id.to_string(),
        name: format!("Step {}", id),
        step_type: "ruleset".to_string(),
        routes: None,
        default: None,
        next: next.map(|n| StepNext::StepId(n.to_string())),
        when: None,
        details: StepDetails::Ruleset {
            ruleset: ruleset.to_string(),
        },
    }
}

/// Helper to create a router step with routes
fn create_router_step(
    id: &str,
    routes: Vec<(&str, Expression)>,
    default: Option<&str>,
) -> PipelineStep {
    let route_objs: Vec<Route> = routes
        .into_iter()
        .map(|(next, condition)| Route {
            next: next.to_string(),
            when: WhenBlock {
                event_type: None,
                condition_group: Some(ConditionGroup::All(vec![Condition::Expression(condition)])),
                conditions: None,
            },
        })
        .collect();

    PipelineStep {
        id: id.to_string(),
        name: format!("Router {}", id),
        step_type: "router".to_string(),
        routes: Some(route_objs),
        default: default.map(|d| d.to_string()),
        next: None,
        when: None,
        details: StepDetails::Unknown {},
    }
}

#[test]
fn test_simple_linear_pipeline() {
    // Create a simple linear pipeline: step1 -> step2 -> end
    let step1 = create_ruleset_step("step1", "ruleset1", Some("step2"));
    let step2 = create_ruleset_step("step2", "ruleset2", Some("end"));

    let pipeline = Pipeline {
        id: "test_pipeline".to_string(),
        name: "Test Linear Pipeline".to_string(),
        description: None,
        entry: "step1".to_string(),
        when: None,
        steps: vec![step1, step2],
        decision: None,
        metadata: None,
    };

    let result = PipelineCompiler::compile(&pipeline);
    assert!(result.is_ok(), "Failed to compile pipeline: {:?}", result.err());

    let program = result.unwrap();
    let instructions = &program.instructions;

    // Expected structure:
    // 1. CallRuleset(ruleset1)
    // 2. Jump to step2
    // 3. CallRuleset(ruleset2)
    // 4. Jump to end
    // 5. Return

    assert!(
        instructions.len() >= 5,
        "Expected at least 5 instructions, got {}",
        instructions.len()
    );

    // Verify first ruleset call
    matches!(
        instructions[0],
        Instruction::CallRuleset { ref ruleset_id } if ruleset_id == "ruleset1"
    );

    // Verify second ruleset call is somewhere in the instructions
    let has_ruleset2 = instructions.iter().any(|inst| {
        matches!(
            inst,
            Instruction::CallRuleset { ref ruleset_id } if ruleset_id == "ruleset2"
        )
    });
    assert!(has_ruleset2, "Expected CallRuleset for ruleset2");

    // Last instruction should be Return
    assert!(
        matches!(instructions.last(), Some(Instruction::Return)),
        "Last instruction should be Return"
    );
}

#[test]
fn test_router_with_multiple_routes() {
    // Create a router with 3 routes + default
    let condition1 = Expression::binary(
        Expression::field_access(vec!["event".to_string(), "amount".to_string()]),
        Operator::Gt,
        Expression::literal(Value::Number(10000.0)),
    );

    let condition2 = Expression::binary(
        Expression::field_access(vec!["event".to_string(), "amount".to_string()]),
        Operator::Gt,
        Expression::literal(Value::Number(1000.0)),
    );

    let router = create_router_step(
        "router1",
        vec![("high", condition1), ("medium", condition2)],
        Some("low"),
    );

    let high_step = create_ruleset_step("high", "high_risk", Some("end"));
    let medium_step = create_ruleset_step("medium", "medium_risk", Some("end"));
    let low_step = create_ruleset_step("low", "low_risk", Some("end"));

    let pipeline = Pipeline {
        id: "router_test".to_string(),
        name: "Router Test Pipeline".to_string(),
        description: None,
        entry: "router1".to_string(),
        when: None,
        steps: vec![router, high_step, medium_step, low_step],
        decision: None,
        metadata: None,
    };

    let result = PipelineCompiler::compile(&pipeline);
    assert!(
        result.is_ok(),
        "Failed to compile router pipeline: {:?}",
        result.err()
    );

    let program = result.unwrap();
    let instructions = &program.instructions;

    // Verify we have LoadField instructions for conditions
    let has_load_field = instructions.iter().any(|inst| {
        matches!(inst, Instruction::LoadField { .. })
    });
    assert!(has_load_field, "Expected LoadField instructions for router conditions");

    // Verify we have Compare instructions
    let has_compare = instructions
        .iter()
        .any(|inst| matches!(inst, Instruction::Compare { .. }));
    assert!(has_compare, "Expected Compare instructions for router conditions");

    // Verify we have JumpIfFalse instructions
    let has_jump_if_false = instructions
        .iter()
        .any(|inst| matches!(inst, Instruction::JumpIfFalse { .. }));
    assert!(
        has_jump_if_false,
        "Expected JumpIfFalse instructions for router"
    );

    // Verify all rulesets are called
    let has_high = instructions.iter().any(|inst| {
        matches!(
            inst,
            Instruction::CallRuleset { ref ruleset_id } if ruleset_id == "high_risk"
        )
    });
    let has_medium = instructions.iter().any(|inst| {
        matches!(
            inst,
            Instruction::CallRuleset { ref ruleset_id } if ruleset_id == "medium_risk"
        )
    });
    let has_low = instructions.iter().any(|inst| {
        matches!(
            inst,
            Instruction::CallRuleset { ref ruleset_id } if ruleset_id == "low_risk"
        )
    });

    assert!(has_high, "Expected CallRuleset for high_risk");
    assert!(has_medium, "Expected CallRuleset for medium_risk");
    assert!(has_low, "Expected CallRuleset for low_risk");
}

#[test]
fn test_sequential_routers() {
    // Test a pipeline with multiple routers in sequence
    let condition1 = Expression::binary(
        Expression::field_access(vec!["event".to_string(), "verified".to_string()]),
        Operator::Eq,
        Expression::literal(Value::Bool(true)),
    );

    let condition2 = Expression::binary(
        Expression::field_access(vec!["event".to_string(), "amount".to_string()]),
        Operator::Gt,
        Expression::literal(Value::Number(5000.0)),
    );

    let router1 = create_router_step("router1", vec![("router2", condition1)], Some("reject"));

    let router2 = create_router_step("router2", vec![("approve", condition2)], Some("manual"));

    let approve = create_ruleset_step("approve", "auto_approve", Some("end"));
    let manual = create_ruleset_step("manual", "manual_review", Some("end"));
    let reject = create_ruleset_step("reject", "auto_reject", Some("end"));

    let pipeline = Pipeline {
        id: "sequential_routers".to_string(),
        name: "Sequential Routers Test".to_string(),
        description: None,
        entry: "router1".to_string(),
        when: None,
        steps: vec![router1, router2, approve, manual, reject],
        decision: None,
        metadata: None,
    };

    let result = PipelineCompiler::compile(&pipeline);
    assert!(
        result.is_ok(),
        "Failed to compile sequential routers: {:?}",
        result.err()
    );

    let program = result.unwrap();
    let instructions = &program.instructions;

    // Verify all three rulesets exist
    let has_approve = instructions.iter().any(|inst| {
        matches!(
            inst,
            Instruction::CallRuleset { ref ruleset_id } if ruleset_id == "auto_approve"
        )
    });
    let has_manual = instructions.iter().any(|inst| {
        matches!(
            inst,
            Instruction::CallRuleset { ref ruleset_id } if ruleset_id == "manual_review"
        )
    });
    let has_reject = instructions.iter().any(|inst| {
        matches!(
            inst,
            Instruction::CallRuleset { ref ruleset_id } if ruleset_id == "auto_reject"
        )
    });

    assert!(has_approve, "Expected auto_approve ruleset");
    assert!(has_manual, "Expected manual_review ruleset");
    assert!(has_reject, "Expected auto_reject ruleset");
}

#[test]
fn test_complex_routing_logic() {
    // Test with complex boolean expressions in router
    let complex_condition = Expression::binary(
        Expression::binary(
            Expression::field_access(vec!["event".to_string(), "amount".to_string()]),
            Operator::Gt,
            Expression::literal(Value::Number(10000.0)),
        ),
        Operator::And,
        Expression::binary(
            Expression::field_access(vec!["event".to_string(), "verified".to_string()]),
            Operator::Eq,
            Expression::literal(Value::Bool(false)),
        ),
    );

    let router = create_router_step("router", vec![("high_risk", complex_condition)], Some("low_risk"));

    let high = create_ruleset_step("high_risk", "high_risk_rules", Some("end"));
    let low = create_ruleset_step("low_risk", "low_risk_rules", Some("end"));

    let pipeline = Pipeline {
        id: "complex_routing".to_string(),
        name: "Complex Routing Test".to_string(),
        description: None,
        entry: "router".to_string(),
        when: None,
        steps: vec![router, high, low],
        decision: None,
        metadata: None,
    };

    let result = PipelineCompiler::compile(&pipeline);
    assert!(
        result.is_ok(),
        "Failed to compile complex routing: {:?}",
        result.err()
    );

    let program = result.unwrap();
    let instructions = &program.instructions;

    // Verify we have BinaryOp instruction for AND
    let has_and = instructions
        .iter()
        .any(|inst| matches!(inst, Instruction::BinaryOp { op } if op == &Operator::And));
    assert!(has_and, "Expected BinaryOp And instruction for complex condition");
}

#[test]
fn test_unreachable_step_in_pipeline() {
    // Create a pipeline where one step is unreachable
    let step1 = create_ruleset_step("step1", "ruleset1", Some("end"));
    let step2 = create_ruleset_step("step2", "ruleset2", Some("end")); // Unreachable

    let pipeline = Pipeline {
        id: "unreachable_test".to_string(),
        name: "Unreachable Step Test".to_string(),
        description: None,
        entry: "step1".to_string(),
        when: None,
        steps: vec![step1, step2],
        decision: None,
        metadata: None,
    };

    // Semantic analyzer should warn about this, but compilation should succeed
    let result = PipelineCompiler::compile(&pipeline);
    assert!(
        result.is_ok(),
        "Compilation should succeed even with unreachable steps: {:?}",
        result.err()
    );

    let program = result.unwrap();
    let instructions = &program.instructions;

    // Step2 should NOT be compiled since it's unreachable
    let has_ruleset2 = instructions.iter().any(|inst| {
        matches!(
            inst,
            Instruction::CallRuleset { ref ruleset_id } if ruleset_id == "ruleset2"
        )
    });
    assert!(
        !has_ruleset2,
        "Unreachable step should not be compiled into instructions"
    );
}

#[test]
fn test_router_without_default() {
    // Test a router that only has routes, no default
    let condition = Expression::binary(
        Expression::field_access(vec!["event".to_string(), "amount".to_string()]),
        Operator::Gt,
        Expression::literal(Value::Number(1000.0)),
    );

    let router = create_router_step("router", vec![("approve", condition)], None);
    let approve = create_ruleset_step("approve", "approve_rules", Some("end"));

    let pipeline = Pipeline {
        id: "no_default_router".to_string(),
        name: "Router Without Default".to_string(),
        description: None,
        entry: "router".to_string(),
        when: None,
        steps: vec![router, approve],
        decision: None,
        metadata: None,
    };

    // Should compile successfully - semantic analyzer would warn about dead end
    let result = PipelineCompiler::compile(&pipeline);
    assert!(
        result.is_ok(),
        "Router without default should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_all_routes_lead_to_end() {
    // Test a pipeline where all paths lead directly to "end"
    let condition = Expression::binary(
        Expression::field_access(vec!["event".to_string(), "valid".to_string()]),
        Operator::Eq,
        Expression::literal(Value::Bool(true)),
    );

    let router = create_router_step("router", vec![("end", condition)], Some("end"));

    let pipeline = Pipeline {
        id: "all_to_end".to_string(),
        name: "All Routes to End".to_string(),
        description: None,
        entry: "router".to_string(),
        when: None,
        steps: vec![router],
        decision: None,
        metadata: None,
    };

    let result = PipelineCompiler::compile(&pipeline);
    assert!(
        result.is_ok(),
        "Pipeline with all routes to end should compile: {:?}",
        result.err()
    );

    let program = result.unwrap();
    let instructions = &program.instructions;

    // Should have minimal instructions - just condition evaluation and jumps to Return
    // Last instruction should be Return
    assert!(
        matches!(instructions.last(), Some(Instruction::Return)),
        "Last instruction should be Return"
    );
}

#[test]
fn test_invalid_step_reference() {
    // Test that referencing a non-existent step ID fails during compilation
    let step1 = create_ruleset_step("step1", "ruleset1", Some("nonexistent"));

    let pipeline = Pipeline {
        id: "invalid_ref".to_string(),
        name: "Invalid Reference Test".to_string(),
        description: None,
        entry: "step1".to_string(),
        when: None,
        steps: vec![step1],
        decision: None,
        metadata: None,
    };

    let result = PipelineCompiler::compile(&pipeline);
    assert!(
        result.is_err(),
        "Should fail when referencing non-existent step"
    );
}

#[test]
fn test_empty_pipeline() {
    // Test edge case: pipeline with entry but no steps
    let pipeline = Pipeline {
        id: "empty".to_string(),
        name: "Empty Pipeline".to_string(),
        description: None,
        entry: "step1".to_string(),
        when: None,
        steps: vec![],
        decision: None,
        metadata: None,
    };

    let result = PipelineCompiler::compile(&pipeline);
    assert!(
        result.is_err(),
        "Empty pipeline should fail compilation"
    );
}
