//! Reproducible engine capacity measurements; stdout is one JSON report.
//! Run via tests/scripts/run_core_benchmark.py for host/RSS evidence.
use corint_decision_engine::{
    CoreSource, DecisionEngine, DecisionRequest, FieldType, Schema, SchemaField, Value,
};
use serde_json::json;
use std::{collections::HashMap, sync::Arc, time::Instant};

fn source(path: &str, value: serde_json::Value) -> CoreSource {
    CoreSource {
        path: path.into(),
        yaml: serde_json::to_string(&value).unwrap(),
    }
}
fn policy(count: usize) -> Vec<CoreSource> {
    let mut sources = Vec::new();
    let names: Vec<_> = (0..count).map(|i| format!("r{i}")).collect();
    for name in &names {
        sources.push(source(&format!("{name}.yaml"),json!({"version":"0.1","rule":{"id":name,"name":name,"when":"event.amount > 0","score":1}})));
    }
    sources.push(source("ruleset.yaml",json!({"version":"0.1","ruleset":{"id":"risk","rules":names,"conclusion":[{"default":true,"signal":"review"}]}})));
    sources.push(source("pipeline.yaml",json!({"version":"0.1","pipeline":{"id":"p","name":"p","entry":"check","steps":[{"step":{"id":"check","name":"check","type":"ruleset","ruleset":"risk","next":"end"}}],"decision":[{"default":true,"result":"review"}]}})));
    sources.push(source(
        "registry.yaml",
        json!({"version":"0.1","registry":[{"pipeline":"p","when":"true"}]}),
    ));
    sources
}
fn schema() -> Schema {
    let mut s = Schema::new("benchmark".into());
    s.fields.insert(
        "amount".into(),
        SchemaField::new("amount".into(), FieldType::Number).required(),
    );
    s
}
fn request(trace: bool) -> DecisionRequest {
    let r = DecisionRequest::new(HashMap::from([("amount".into(), Value::Number(1.0))]));
    if trace {
        r.with_trace()
    } else {
        r
    }
}
fn percentile(sorted: &[f64], percent: usize) -> f64 {
    sorted[(sorted.len() * percent).div_ceil(100).saturating_sub(1)]
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let samples = args
        .first()
        .map(|v| v.parse::<usize>().expect("samples must be integer"))
        .unwrap_or(1000);
    assert!(
        (100..=100_000).contains(&samples),
        "samples must be 100–100000"
    );
    let mut results = Vec::new();
    for rules in [16, 128, 512] {
        let sources = policy(rules);
        let start = Instant::now();
        let engine = Arc::new(DecisionEngine::from_core(&sources, schema()).unwrap());
        let compile_ms = start.elapsed().as_secs_f64() * 1000.0;
        for trace in [false, true] {
            for _ in 0..20 {
                assert_eq!(
                    engine.decide(request(trace)).await.unwrap().result.score,
                    rules as i32
                );
            }
            for concurrency in [1, 4, 16] {
                let start = Instant::now();
                let mut tasks = Vec::new();
                for worker in 0..concurrency {
                    let engine = engine.clone();
                    tasks.push(tokio::spawn(async move {
                        let mut timings = Vec::new();
                        for _ in (worker..samples).step_by(concurrency) {
                            let start = Instant::now();
                            let result = engine.decide(request(trace)).await.unwrap();
                            assert_eq!(result.result.score, rules as i32);
                            timings.push(start.elapsed().as_secs_f64() * 1_000_000.0);
                        }
                        timings
                    }));
                }
                let mut timings = Vec::new();
                for task in tasks {
                    timings.extend(task.await.unwrap());
                }
                let seconds = start.elapsed().as_secs_f64();
                timings.sort_by(f64::total_cmp);
                results.push(json!({"rules":rules,"samples":samples,"concurrency":concurrency,"trace":trace,"compile_ms":compile_ms,"p50_us":percentile(&timings,50),"p95_us":percentile(&timings,95),"p99_us":percentile(&timings,99),"requests_per_second":samples as f64/seconds}));
            }
        }
    }
    println!("{}",serde_json::to_string(&json!({"format_version":"1","scope":"synthetic_in_process_core","build_mode":if cfg!(debug_assertions){"debug"}else{"release"},"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"worker_threads":4,"warmup_requests":20,"measurements":results})).unwrap());
}
