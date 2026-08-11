use std::{env, fs, path::PathBuf};

use anyhow::{Context, Result, bail};

fn main() -> Result<()> {
    let mut arguments = env::args().skip(1);
    let check = arguments
        .next()
        .is_some_and(|argument| argument == "--check");
    let output = if check {
        arguments.next()
    } else {
        env::args().nth(1)
    }
    .map(PathBuf::from)
    .unwrap_or_else(|| PathBuf::from("packages/api-contract/openapi.json"));
    if arguments.next().is_some() {
        bail!("usage: export-openapi [--check] [output]");
    }

    let mut json = serde_json::to_string_pretty(&server_admin_rs::api_contract_document())
        .context("serialize OpenAPI document")?;
    json.push('\n');
    if check {
        let current = fs::read_to_string(&output)
            .with_context(|| format!("read checked contract {}", output.display()))?;
        if current != json {
            bail!("{} is stale; run npm run api:generate", output.display());
        }
        println!("[api-contract] {} is current", output.display());
    } else {
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        }
        fs::write(&output, json)
            .with_context(|| format!("write OpenAPI contract {}", output.display()))?;
        println!("[api-contract] wrote {}", output.display());
    }
    Ok(())
}
