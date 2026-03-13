use std::collections::HashMap;

use docmost_local_mcp::startup_config::{normalize_base_url, parse_startup_config};

#[test]
fn reads_base_url_from_cli_arguments() {
    let argv = vec![
        "--base-url".to_string(),
        "https://docs.example.com/".to_string(),
    ];
    let config = parse_startup_config(&argv, &HashMap::new()).unwrap();
    assert_eq!(config.base_url.as_deref(), Some("https://docs.example.com"));
}

#[test]
fn supports_inline_cli_argument_syntax() {
    let argv = vec!["--base-url=https://docs.example.com/".to_string()];
    let config = parse_startup_config(&argv, &HashMap::new()).unwrap();
    assert_eq!(config.base_url.as_deref(), Some("https://docs.example.com"));
}

#[test]
fn falls_back_to_environment() {
    let env = HashMap::from([(
        "DOCMOST_BASE_URL".to_string(),
        "https://env.example.com/".to_string(),
    )]);
    let config = parse_startup_config(&[], &env).unwrap();
    assert_eq!(config.base_url.as_deref(), Some("https://env.example.com"));
}

#[test]
fn throws_when_base_url_flag_is_missing_value() {
    let argv = vec!["--base-url".to_string()];
    let error = parse_startup_config(&argv, &HashMap::new()).unwrap_err();
    assert_eq!(error.to_string(), "Missing value for --base-url.");
}

#[test]
fn removes_trailing_slashes() {
    assert_eq!(
        normalize_base_url("https://docs.example.com///"),
        "https://docs.example.com"
    );
}
