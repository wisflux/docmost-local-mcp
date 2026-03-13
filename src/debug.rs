use chrono::Utc;
use serde::Serialize;

pub fn debug_log<T>(scope: &str, message: &str, details: Option<&T>)
where
    T: Serialize + ?Sized,
{
    if !debug_enabled() {
        return;
    }

    let timestamp = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let prefix = format!("[docmost-local-mcp][{timestamp}][{scope}]");

    match details {
        Some(details) => {
            let serialized =
                serde_json::to_string(details).unwrap_or_else(|_| "<unserializable>".to_string());
            eprintln!("{prefix} {message} {serialized}");
        }
        None => eprintln!("{prefix} {message}"),
    }
}

pub fn debug_enabled() -> bool {
    matches!(
        std::env::var("DEBUG_DOCMOST_MCP").ok().as_deref(),
        Some("1") | Some("true")
    )
}
