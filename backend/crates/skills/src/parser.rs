use anyhow::{bail, Context, Result};
use serde::Deserialize;

/// Parsed content of a SKILL.md file.
#[derive(Debug, Clone)]
pub struct ParsedSkill {
    pub name:          String,
    pub description:   String,
    pub system_prompt: String,
    pub tool_hints:    Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Frontmatter {
    name:        String,
    description: String,
}

/// Parse a raw SKILL.md string into a `ParsedSkill`.
///
/// Format:
/// ```text
/// ---
/// name: my-skill
/// description: When to use this skill
/// ---
///
/// # Skill content (becomes system_prompt)
/// ```
pub fn parse_skill_md(content: &str) -> Result<ParsedSkill> {
    // Must start with ---
    if !content.trim_start().starts_with("---") {
        bail!("SKILL.md must begin with YAML frontmatter (---)")
    }

    let content = content.trim_start();
    // Find the closing ---
    let rest = &content[3..]; // skip opening ---
    let end = rest.find("\n---")
        .context("SKILL.md missing closing '---' frontmatter delimiter")?;

    let yaml_part = &rest[..end];
    let body_start = end + 4; // skip \n---
    let system_prompt = rest[body_start..].trim().to_string();

    let frontmatter: Frontmatter = serde_yaml::from_str(yaml_part)
        .context("Failed to parse SKILL.md frontmatter YAML")?;

    if frontmatter.name.is_empty() {
        bail!("SKILL.md frontmatter must have a non-empty 'name' field");
    }
    if frontmatter.description.is_empty() {
        bail!("SKILL.md frontmatter must have a non-empty 'description' field");
    }

    let tool_hints = extract_tool_hints(&system_prompt);

    Ok(ParsedSkill {
        name:          frontmatter.name,
        description:   frontmatter.description,
        system_prompt,
        tool_hints,
    })
}

/// Scan the skill body for references to known tool names.
fn extract_tool_hints(body: &str) -> Vec<String> {
    let known_tools = [
        "file_read", "file_write", "file_list",
        "code_execute", "web_search", "web_fetch",
        "shell_command", "mcp_call",
    ];
    known_tools
        .iter()
        .filter(|&&t| body.contains(t))
        .map(|&t| t.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_skill() {
        let md = r#"---
name: senior-developer
description: Use when writing or reviewing code
---

# Senior Developer

You are an expert engineer. Use file_read and file_write to work with files.
"#;
        let skill = parse_skill_md(md).unwrap();
        assert_eq!(skill.name, "senior-developer");
        assert_eq!(skill.description, "Use when writing or reviewing code");
        assert!(skill.system_prompt.contains("expert engineer"));
        assert!(skill.tool_hints.contains(&"file_read".to_string()));
        assert!(skill.tool_hints.contains(&"file_write".to_string()));
    }

    #[test]
    fn error_on_missing_frontmatter() {
        let md = "# No frontmatter here";
        assert!(parse_skill_md(md).is_err());
    }
}
