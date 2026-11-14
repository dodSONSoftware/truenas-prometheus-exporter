use anyhow::{Result, bail};
use truenas_exporter::{api::chart_comparison, config::Config};
#[tokio::main]
async fn main() -> Result<()> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("output path required"))?;
    let output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    let mut config = Config::from_env()?;
    if config.targets.len() != 1 {
        bail!("exactly one target required");
    }
    let api = config
        .targets
        .remove(0)
        .api
        .ok_or_else(|| anyhow::anyhow!("live configuration required"))?;
    let result = chart_comparison::run(api)
        .await
        .map_err(|e| anyhow::anyhow!("comparison failed: {}", e.as_str()))?;
    serde_json::to_writer_pretty(output, &result)?;
    if !result["error"].is_null() {
        bail!("comparison incomplete; sanitized evidence saved");
    }
    println!("Bounded comparison completed; sanitized evidence saved");
    Ok(())
}
