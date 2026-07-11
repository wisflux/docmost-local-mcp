//! Docmost server version detection and version-gated capabilities.
//!
//! Behaviour differs across Docmost versions, so the client detects the server version
//! once (via `POST /api/version`, which returns `{ data: { currentVersion } }`) and
//! derives a [`Capabilities`] set from it. When the version can't be determined (endpoint
//! unavailable, e.g. Docmost Cloud, or a network error) we assume the **conservative**
//! (older) behaviour so a tool never claims a capability the server may lack.
//!
//! Thresholds are grounded in the Docmost source at tagged releases (the version line is
//! `… v0.25.3 → v0.70.0 … v0.95.0`, with nothing in between):
//! - REST page-body updates (`/api/pages/update` `content`) were added in **v0.70.0**;
//!   on ≤ v0.25.x the update DTO has no `content` field and the body is edited only
//!   through the collaborative editor.

use serde::Deserialize;

/// A parsed `major.minor.patch` Docmost server version. Ordering is numeric per component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ServerVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl ServerVersion {
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Parse a version string like `"0.25.3"` (ignoring any leading `v` or trailing
    /// pre-release/build suffix). Returns `None` if the major/minor can't be read.
    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim().trim_start_matches('v');
        let core = raw
            .split(['-', '+'])
            .next()
            .unwrap_or(raw)
            .trim_matches('"');
        let mut parts = core.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        Some(Self::new(major, minor, patch))
    }
}

impl std::fmt::Display for ServerVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Docmost added REST page-body updates in v0.70.0.
pub const REST_PAGE_BODY_UPDATE_MIN: ServerVersion = ServerVersion::new(0, 70, 0);

/// Oldest Docmost version this project targets — roughly the last year of releases
/// (v0.22.0 shipped mid-2025). Older servers still work on a best-effort basis
/// (capability detection applies), but their version-specific quirks aren't handled.
pub const MIN_SUPPORTED_VERSION: ServerVersion = ServerVersion::new(0, 22, 0);

/// Version-gated server capabilities, derived from the detected [`ServerVersion`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    /// Whether `POST /api/pages/update` with a `content` body actually persists the body.
    /// On older servers the body lives in the collaborative `ydoc` and a REST body update
    /// is silently ignored.
    pub rest_page_body_update: bool,
}

impl Capabilities {
    /// Derive capabilities from a detected version. An unknown version (`None`) yields the
    /// conservative set — we never claim a capability the server might not have.
    pub fn for_version(version: Option<ServerVersion>) -> Self {
        Self {
            rest_page_body_update: version.is_some_and(|v| v >= REST_PAGE_BODY_UPDATE_MIN),
        }
    }
}

/// Response shape of `POST /api/version` (unwrapped from the `{ data: ... }` envelope).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionResponse {
    pub current_version: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_and_decorated_versions() {
        assert_eq!(
            ServerVersion::parse("0.25.3"),
            Some(ServerVersion::new(0, 25, 3))
        );
        assert_eq!(
            ServerVersion::parse("v0.70.0"),
            Some(ServerVersion::new(0, 70, 0))
        );
        assert_eq!(
            ServerVersion::parse("0.95"),
            Some(ServerVersion::new(0, 95, 0))
        );
        assert_eq!(
            ServerVersion::parse("1.2.3-beta.1"),
            Some(ServerVersion::new(1, 2, 3))
        );
        assert_eq!(ServerVersion::parse("not-a-version"), None);
    }

    #[test]
    fn version_ordering_is_numeric_not_lexical() {
        // "0.9.0" must be less than "0.70.0" numerically (lexical string compare would flip).
        assert!(ServerVersion::new(0, 9, 0) < ServerVersion::new(0, 70, 0));
        assert!(ServerVersion::new(0, 25, 3) < ServerVersion::new(0, 70, 0));
        assert!(ServerVersion::new(0, 70, 0) <= ServerVersion::new(0, 70, 0));
    }

    #[test]
    fn body_update_capability_gates_on_0_70_0() {
        let cap = |v| Capabilities::for_version(v).rest_page_body_update;
        assert!(
            !cap(Some(ServerVersion::new(0, 25, 3))),
            "0.25.3 lacks REST body update"
        );
        assert!(cap(Some(ServerVersion::new(0, 70, 0))), "0.70.0 has it");
        assert!(cap(Some(ServerVersion::new(0, 95, 0))), "0.95.0 has it");
        assert!(
            !cap(None),
            "unknown version => conservative (no capability)"
        );
    }
}
