use once_cell::sync::Lazy;
use regex::Regex;

use crate::types::{
    DocmostComment, DocmostCurrentUserResponse, DocmostPage, DocmostPageListItem,
    DocmostSearchResult, DocmostUser,
};

static HIGHLIGHT_TAGS_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"<[^>]+>").expect("valid highlight strip regex"));
static COLLAPSE_WHITESPACE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\s+").expect("valid whitespace collapse regex"));

pub fn sanitize_highlight(value: Option<&str>) -> String {
    let Some(value) = value else {
        return String::new();
    };
    COLLAPSE_WHITESPACE_RE
        .replace_all(&HIGHLIGHT_TAGS_RE.replace_all(value, ""), " ")
        .trim()
        .to_string()
}

pub fn format_search_results(query: &str, results: &[DocmostSearchResult]) -> String {
    if results.is_empty() {
        return format!("No Docmost results were found for \"{query}\".");
    }

    let mut lines = vec![format!("## Search Results for \"{query}\""), String::new()];
    let total_results = results.len();

    for (index, result) in results.iter().take(5).enumerate() {
        let space_name = result
            .space
            .as_ref()
            .and_then(|space| space.name.as_deref())
            .unwrap_or("Unknown");
        let preview = sanitize_highlight(result.highlight.as_deref());
        let icon = result.icon.as_deref().unwrap_or("");
        let title = result.title.as_deref().unwrap_or("Untitled");

        if icon.is_empty() {
            lines.push(format!("### {}. {}", index + 1, title));
        } else {
            lines.push(format!("### {}. {} {}", index + 1, icon, title));
        }
        lines.push(format!("- Space: {space_name}"));
        lines.push(format!(
            "- Page ID: {}",
            format_optional_id(result.id.as_deref())
        ));
        lines.push(format!("- Slug ID: `{}`", result.slug_id));
        if !preview.is_empty() {
            lines.push(format!("- Preview: {preview}"));
        }
        lines.push(String::new());
    }

    lines.push(format!(
        "Showing {} of {} results.",
        results.iter().take(5).count(),
        total_results
    ));
    lines.push("Use `get_page` with a slug ID to retrieve the full page.".to_string());
    lines.join("\n")
}

pub fn format_page_list(title: &str, scope: &str, pages: &[DocmostPageListItem]) -> String {
    if pages.is_empty() {
        return format!("No Docmost pages were found for {scope}.");
    }

    let mut lines = vec![format!("## {title}"), String::new()];
    for (index, page) in pages.iter().take(10).enumerate() {
        let icon = page.icon.as_deref().unwrap_or("");
        let title = page.title.as_deref().unwrap_or("Untitled");
        if icon.is_empty() {
            lines.push(format!("### {}. {}", index + 1, title));
        } else {
            lines.push(format!("### {}. {} {}", index + 1, icon, title));
        }
        lines.push(format!("- Page ID: `{}`", page.id));
        lines.push(format!("- Slug ID: `{}`", page.slug_id));
        lines.push(format!(
            "- Parent Page ID: {}",
            format_optional_id(page.parent_page_id.as_deref())
        ));
        lines.push(format!(
            "- Has Children: {}",
            page.has_children.unwrap_or(false)
        ));
        lines.push(String::new());
    }
    lines.push(format!(
        "Showing {} of {} pages.",
        pages.iter().take(10).count(),
        pages.len()
    ));
    lines.join("\n")
}

pub fn format_comments(page_id: &str, comments: &[DocmostComment]) -> String {
    if comments.is_empty() {
        return format!("No comments were found for page `{page_id}`.");
    }

    let mut lines = vec![format!("## Comments for Page `{page_id}`"), String::new()];
    for (index, comment) in comments.iter().take(10).enumerate() {
        let author = comment
            .creator
            .as_ref()
            .and_then(|user| user.name.as_deref())
            .unwrap_or("Unknown");
        lines.push(format!("### {}. Comment `{}`", index + 1, comment.id));
        lines.push(format!("- Author: {author}"));
        lines.push(format!(
            "- Parent Comment ID: {}",
            format_optional_id(comment.parent_comment_id.as_deref())
        ));
        lines.push(format!(
            "- Selection: {}",
            comment.selection.as_deref().unwrap_or("None")
        ));
        lines.push(format!(
            "- Resolved: {}",
            if comment.resolved_at.is_some() {
                "Yes"
            } else {
                "No"
            }
        ));
        lines.push(String::new());
    }
    lines.push(format!(
        "Showing {} of {} comments.",
        comments.iter().take(10).count(),
        comments.len()
    ));
    lines.join("\n")
}

pub fn format_workspace_members(members: &[DocmostUser]) -> String {
    if members.is_empty() {
        return "No Docmost workspace members were found.".to_string();
    }

    let mut lines = vec![
        "## Workspace Members".to_string(),
        String::new(),
        "| Name | Email | Role | ID |".to_string(),
        "| --- | --- | --- | --- |".to_string(),
    ];

    for member in members.iter().take(20) {
        lines.push(format!(
            "| {} | {} | {} | {} |",
            member.name.as_deref().unwrap_or("Unknown"),
            member.email.as_deref().unwrap_or("Unknown"),
            member.role.as_deref().unwrap_or("Unknown"),
            member.id
        ));
    }

    lines.push(String::new());
    lines.push(format!(
        "Showing {} of {} members.",
        members.iter().take(20).count(),
        members.len()
    ));
    lines.join("\n")
}

pub fn format_current_user(response: &DocmostCurrentUserResponse) -> String {
    let lines = [
        "# Current Docmost User".to_string(),
        String::new(),
        format!(
            "Name: {}",
            response.user.name.as_deref().unwrap_or("Unknown")
        ),
        format!("User ID: `{}`", response.user.id),
        format!(
            "Email: {}",
            response.user.email.as_deref().unwrap_or("Unknown")
        ),
        format!(
            "Role: {}",
            response.user.role.as_deref().unwrap_or("Unknown")
        ),
        String::new(),
        "## Workspace".to_string(),
        String::new(),
        format!(
            "Name: {}",
            response.workspace.name.as_deref().unwrap_or("Unknown")
        ),
        format!("Workspace ID: `{}`", response.workspace.id),
        format!(
            "Hostname: {}",
            response.workspace.hostname.as_deref().unwrap_or("Unknown")
        ),
        format!(
            "Member count: {}",
            response
                .workspace
                .member_count
                .map(|count| count.to_string())
                .unwrap_or_else(|| "Unknown".to_string())
        ),
    ];

    lines.join("\n")
}

pub fn format_created_page(page: &DocmostPage, requested_title: &str) -> String {
    let title = page.title.as_deref().unwrap_or(requested_title);
    let lines = [
        format!("Created Docmost page \"{title}\"."),
        String::new(),
        format!("Page ID: {}", format_optional_id(page.id.as_deref())),
        format!("Slug ID: {}", format_optional_id(page.slug_id.as_deref())),
        format!("Space ID: {}", format_optional_id(page.space_id.as_deref())),
    ];
    lines.join("\n")
}

pub fn format_updated_page(page: &DocmostPage) -> String {
    let title = page.title.as_deref().unwrap_or("Untitled");
    let lines = [
        format!("Updated Docmost page \"{title}\"."),
        String::new(),
        format!("Page ID: {}", format_optional_id(page.id.as_deref())),
        format!("Slug ID: {}", format_optional_id(page.slug_id.as_deref())),
        "Note: content changes are applied via the collaborative editor and may take a moment to fully persist."
            .to_string(),
    ];
    lines.join("\n")
}

fn format_optional_id(value: Option<&str>) -> String {
    value
        .map(|value| format!("`{value}`"))
        .unwrap_or_else(|| "Unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn page(value: serde_json::Value) -> DocmostPage {
        serde_json::from_value(value).expect("valid DocmostPage")
    }

    #[test]
    fn format_created_page_uses_returned_title_and_ids() {
        let output = format_created_page(
            &page(json!({
                "id": "p1",
                "slugId": "s1",
                "title": "Release Notes",
                "spaceId": "space-1"
            })),
            "requested title",
        );
        assert!(output.contains("Created Docmost page \"Release Notes\"."));
        assert!(output.contains("Page ID: `p1`"));
        assert!(output.contains("Slug ID: `s1`"));
        assert!(output.contains("Space ID: `space-1`"));
    }

    #[test]
    fn format_created_page_falls_back_to_requested_title() {
        // The import endpoint sometimes returns no title; the caller's title is used.
        let output = format_created_page(&page(json!({ "id": "p1" })), "My Requested Title");
        assert!(
            output.contains("Created Docmost page \"My Requested Title\"."),
            "output: {output}"
        );
    }

    #[test]
    fn format_created_page_marks_missing_ids_unknown() {
        let output = format_created_page(&page(json!({ "title": "T" })), "T");
        assert!(output.contains("Page ID: Unknown"), "output: {output}");
        assert!(output.contains("Slug ID: Unknown"), "output: {output}");
        assert!(output.contains("Space ID: Unknown"), "output: {output}");
    }

    #[test]
    fn format_updated_page_reports_title_and_collaborative_note() {
        let output = format_updated_page(&page(json!({
            "id": "p1",
            "slugId": "s1",
            "title": "Renamed"
        })));
        assert!(output.contains("Updated Docmost page \"Renamed\"."));
        assert!(output.contains("Page ID: `p1`"));
        // Documents current behavior: the collaborative-editor note is always present,
        // including for title-only updates where no body was sent.
        assert!(
            output.contains("applied via the collaborative editor"),
            "output: {output}"
        );
    }

    #[test]
    fn format_updated_page_falls_back_to_untitled() {
        let output = format_updated_page(&page(json!({ "id": "p1" })));
        assert!(
            output.contains("Updated Docmost page \"Untitled\"."),
            "output: {output}"
        );
    }
}
