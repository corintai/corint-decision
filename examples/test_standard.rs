use corint_sdk::builder::DecisionEngineBuilder;
use corint_sdk::decision_engine::DecisionRequest;
use corint_sdk::Value;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter("corint_runtime::engine::pipeline_executor=trace".parse()?)
        .init();

    let engine = DecisionEngineBuilder::new()
        .add_rule_file("examples/rules/payment_pipeline.yaml")
        .build()
        .await?;

    let mut event_data = HashMap::new();
    event_data.insert("payment_amount".to_string(), Value::Number(500.0));
    event_data.insert("country_code".to_string(), Value::String("US".to_string()));
    event_data.insert("payment_attempts_1h".to_string(), Value::Number(2.0));
    event_data.insert("unique_cards_1h".to_string(), Value::Number(1.0));
    event_data.insert("transaction_count_24h".to_string(), Value::Number(5.0));
    event_data.insert("account_age_days".to_string(), Value::Number(30.0));
    event_data.insert("is_disposable_email".to_string(), Value::Bool(false));

    let request = DecisionRequest::new(event_data);
    let response = engine.decide(request).await?;

    println!("\nResult:");
    println!("  Action: {:?}", response.result.action);
    println!("  Score: {}", response.result.score);

    Ok(())
}
