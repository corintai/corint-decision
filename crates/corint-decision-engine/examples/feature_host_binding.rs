//! Print the effective input-resource binding without connecting to a datasource.
use corint_decision_engine::decision_host::FeatureHostConfig;
use std::io::Read;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("usage: feature_host_binding CONFIG.json")?;
    let file = std::fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(8 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("configuration exceeds 8 MiB".into());
    }
    let config: FeatureHostConfig =
        serde_json::from_slice(&bytes).map_err(|_| "invalid feature host configuration")?;
    println!("{}", config.binding_sha256());
    Ok(())
}
