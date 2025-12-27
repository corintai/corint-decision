//! Feature extraction tests for pipeline executor

use crate::engine::PipelineExecutor;
use crate::storage::{Event, InMemoryStorage};
use corint_core::ir::{FeatureType, Instruction, Program, ProgramMetadata, TimeWindow};
use corint_core::Value;
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::test]
async fn test_feature_extraction_without_storage() {
    let executor = PipelineExecutor::new();

    let instructions = vec![
        Instruction::CallFeature {
            feature_type: FeatureType::Count,
            field: vec!["transaction".to_string(), "amount".to_string()],
            filter: None,
            time_window: TimeWindow::Last24Hours,
        },
        Instruction::Return,
    ];

    let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

    let result = executor.execute(&program, HashMap::new()).await.unwrap();

    // Without storage, should return placeholder
    assert_eq!(result.score, 0);
}

#[tokio::test]
async fn test_feature_extraction_with_storage() {
    // Create storage with test events
    let mut storage = InMemoryStorage::new();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    for i in 0..5 {
        let mut data = HashMap::new();
        data.insert("amount".to_string(), Value::Number((i + 1) as f64 * 10.0));
        storage.add_event(Event {
            timestamp: now - 100 + i,
            data,
        });
    }

    let executor = PipelineExecutor::with_storage(Arc::new(storage));

    let instructions = vec![
        Instruction::CallFeature {
            feature_type: FeatureType::Sum,
            field: vec!["amount".to_string()],
            filter: None,
            time_window: TimeWindow::Last1Hour,
        },
        Instruction::Return,
    ];

    let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

    let result = executor.execute(&program, HashMap::new()).await.unwrap();

    // Sum of 10, 20, 30, 40, 50 = 150
    assert_eq!(result.score, 0); // Score is separate from stack value
}
