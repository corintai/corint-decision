//! Basic usage example for corint-core
//!
//! Run with: cargo run --example basic_usage

use corint_core::ast::{Expression, Operator, Rule, WhenBlock};
use corint_core::Value;

fn main() {
    println!("=== CORINT Core Basic Usage Example ===\n");

    // Example 1: Creating values
    println!("1. Creating Values:");
    let number = Value::Number(42.0);
    let string = Value::String("hello".to_string());
    let boolean = Value::Bool(true);

    println!("   Number: {:?}", number);
    println!("   String: {:?}", string);
    println!("   Boolean: {:?}\n", boolean);

    // Example 2: Creating a simple expression (user.age > 18)
    println!("2. Creating Expression (user.age > 18):");
    let age_check = Expression::binary(
        Expression::field_access(vec!["user".to_string(), "age".to_string()]),
        Operator::Gt,
        Expression::literal(Value::Number(18.0)),
    );
    println!("   Expression: {:#?}\n", age_check);

    // Example 3: Creating a complex expression ((amount > 1000) && (country == "US"))
    println!("3. Creating Complex Expression:");
    let complex_expr = Expression::binary(
        Expression::binary(
            Expression::field_access(vec!["amount".to_string()]),
            Operator::Gt,
            Expression::literal(Value::Number(1000.0)),
        ),
        Operator::And,
        Expression::binary(
            Expression::field_access(vec!["country".to_string()]),
            Operator::Eq,
            Expression::literal(Value::String("US".to_string())),
        ),
    );
    println!("   Expression: {:#?}\n", complex_expr);

    // Example 4: Creating a function call (count(user.logins, last_7d))
    println!("4. Creating Function Call:");
    let func_call = Expression::function_call(
        "count".to_string(),
        vec![
            Expression::field_access(vec!["user".to_string(), "logins".to_string()]),
            Expression::literal(Value::String("last_7d".to_string())),
        ],
    );
    println!("   Function Call: {:#?}\n", func_call);

    // Example 5: Creating a complete rule
    println!("5. Creating a Complete Rule:");
    let when = WhenBlock::new()
        .with_event_type("login".to_string())
        .add_condition(Expression::binary(
            Expression::field_access(vec!["user".to_string(), "age".to_string()]),
            Operator::Gt,
            Expression::literal(Value::Number(60.0)),
        ))
        .add_condition(Expression::binary(
            Expression::field_access(vec!["device".to_string(), "is_new".to_string()]),
            Operator::Eq,
            Expression::literal(Value::Bool(true)),
        ));

    let rule = Rule::new(
        "high_risk_senior_new_device".to_string(),
        "High Risk Senior with New Device".to_string(),
        when,
        80,
    )
    .with_description("Detects seniors using new devices".to_string());

    println!("   Rule ID: {}", rule.id);
    println!("   Rule Name: {}", rule.name);
    println!("   Rule Score: {}", rule.score);
    println!("   Description: {:?}", rule.description);
    println!("   Event Type: {:?}", rule.when.event_type);
    println!("   Number of Conditions: {}", rule.when.conditions.as_ref().map(|c| c.len()).unwrap_or(0));
    println!("\n   Full Rule: {:#?}\n", rule);

    // Example 6: JSON serialization
    println!("6. JSON Serialization:");
    let value = Value::Object({
        let mut map = std::collections::HashMap::new();
        map.insert("name".to_string(), Value::String("Alice".to_string()));
        map.insert("age".to_string(), Value::Number(25.0));
        map.insert("active".to_string(), Value::Bool(true));
        map
    });

    let json = serde_json::to_string_pretty(&value).unwrap();
    println!("   JSON:\n{}\n", json);

    println!("=== Example Complete ===");
}
