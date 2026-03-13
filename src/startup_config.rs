use std::collections::HashMap;

use anyhow::{Result, bail};

use crate::types::StartupConfig;

pub fn parse_startup_config(
    argv: &[String],
    env: &HashMap<String, String>,
) -> Result<StartupConfig> {
    let mut base_url = read_base_url_from_env(env);
    let mut index = 0usize;

    while index < argv.len() {
        let argument = &argv[index];

        if argument == "--base-url" {
            let value = argv
                .get(index + 1)
                .ok_or_else(|| anyhow::anyhow!("Missing value for --base-url."))?;
            base_url = Some(value.clone());
            index += 2;
            continue;
        }

        if let Some(value) = argument.strip_prefix("--base-url=") {
            base_url = Some(value.to_string());
        }

        index += 1;
    }

    let mut config = StartupConfig::default();
    if let Some(base_url) = base_url.filter(|value| !value.trim().is_empty()) {
        config.base_url = Some(normalize_base_url(&base_url));
    }

    Ok(config)
}

pub fn parse_runtime_startup_config(argv: &[String]) -> Result<StartupConfig> {
    let env = std::env::vars().collect::<HashMap<_, _>>();
    parse_startup_config(argv, &env)
}

pub fn normalize_base_url(base_url: &str) -> String {
    base_url.trim().trim_end_matches('/').to_string()
}

fn read_base_url_from_env(env: &HashMap<String, String>) -> Option<String> {
    let value = env.get("DOCMOST_BASE_URL")?.trim();
    if value.is_empty() {
        return None;
    }

    Some(value.to_string())
}

pub fn ensure_base_url(config: &StartupConfig) -> Result<String> {
    if let Some(base_url) = &config.base_url {
        return Ok(base_url.clone());
    }

    bail!("A Docmost base URL is required. Pass --base-url or set DOCMOST_BASE_URL.")
}
