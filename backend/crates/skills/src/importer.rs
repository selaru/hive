use crate::parser::{parse_skill_md, ParsedSkill};
use anyhow::{Context, Result};
use reqwest::Client;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

pub struct SkillImporter {
    http: Client,
}

#[derive(Debug, Clone)]
pub struct SkillScanResult {
    pub skill: ParsedSkill,
    pub source_path: PathBuf,
    pub source_type: String,
}

impl SkillImporter {
    pub fn new() -> Self {
        Self {
            http: Client::builder()
                .user_agent("hive-skill-importer/0.1")
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .expect("Failed to build HTTP client"),
        }
    }

    /// Import a skill from a GitHub repo (`owner/repo` or full URL).
    pub async fn from_github(&self, repo: &str) -> Result<ParsedSkill> {
        let slug = normalise_github_slug(repo);
        info!("Importing skill from GitHub: {}", slug);

        // Try several common locations
        let candidates = vec![
            format!("https://raw.githubusercontent.com/{}/main/SKILL.md", slug),
            format!("https://raw.githubusercontent.com/{}/master/SKILL.md", slug),
            format!("https://raw.githubusercontent.com/{}/main/.agents/skills/SKILL.md", slug),
        ];

        for url in &candidates {
            debug!("Trying {}", url);
            match self.fetch_text(url).await {
                Ok(content) => {
                    return parse_skill_md(&content)
                        .context(format!("Failed to parse SKILL.md from {}", url));
                }
                Err(_) => continue,
            }
        }

        anyhow::bail!(
            "Could not find SKILL.md in {} — tried main/SKILL.md, master/SKILL.md, \
             and main/.agents/skills/SKILL.md",
            slug
        )
    }

    /// Import from skills.sh slug (resolves to GitHub).
    pub async fn from_skills_sh(&self, slug: &str) -> Result<ParsedSkill> {
        info!("Importing skill from skills.sh slug: {}", slug);
        // skills.sh skills are backed by GitHub repos
        self.from_github(slug).await
    }

    /// Parse a locally pasted SKILL.md string.
    pub fn from_content(&self, content: &str) -> Result<ParsedSkill> {
        parse_skill_md(content)
    }

    /// Scan a directory recursively for SKILL.md files.
    /// 
    /// Supports both OpenCode style (~/.agents/skills/*/SKILL.md) 
    /// and Multica style (workspaces/*/.opencode/skills/*/SKILL.md)
    pub fn scan_directory(&self, dir: &Path) -> Vec<SkillScanResult> {
        if !dir.exists() {
            warn!("Skill directory does not exist: {}", dir.display());
            return vec![];
        }

        info!("Scanning directory for skills: {}", dir.display());
        let mut results = Vec::new();

        if let Err(e) = self.scan_recursive(dir, &mut results) {
            warn!("Error during directory scan: {}", e);
        }

        info!("Found {} skills in {}", results.len(), dir.display());
        results
    }

    fn scan_recursive(&self, dir: &Path, results: &mut Vec<SkillScanResult>) -> Result<()> {
        let entries = std::fs::read_dir(dir)
            .context(format!("Failed to read directory: {}", dir.display()))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() && path.file_name().and_then(|n| n.to_str()) == Some("SKILL.md") {
                match self.load_skill_from_file(&path) {
                    Ok(skill) => {
                        debug!("Loaded skill '{}' from {}", skill.name, path.display());
                        results.push(SkillScanResult {
                            skill,
                            source_path: path.clone(),
                            source_type: "filesystem".to_string(),
                        });
                    }
                    Err(e) => {
                        warn!("Failed to parse {}: {}", path.display(), e);
                    }
                }
            } else if path.is_dir() {
                // Recurse into subdirectories
                let _ = self.scan_recursive(&path, results);
            }
        }

        Ok(())
    }

    fn load_skill_from_file(&self, path: &Path) -> Result<ParsedSkill> {
        let content = std::fs::read_to_string(path)
            .context(format!("Failed to read file: {}", path.display()))?;
        parse_skill_md(&content)
    }

    async fn fetch_text(&self, url: &str) -> Result<String> {
        let resp = self.http.get(url).send().await
            .context(format!("HTTP GET failed: {}", url))?;

        if !resp.status().is_success() {
            anyhow::bail!("HTTP {} for {}", resp.status(), url);
        }

        resp.text().await.context("Failed to read response body")
    }
}

impl Default for SkillImporter {
    fn default() -> Self { Self::new() }
}

/// Normalise various forms of GitHub reference to `owner/repo`.
fn normalise_github_slug(input: &str) -> String {
    let s = input.trim();
    // Strip https://github.com/
    let s = s.strip_prefix("https://github.com/").unwrap_or(s);
    let s = s.strip_prefix("http://github.com/").unwrap_or(s);
    // Strip trailing .git
    let s = s.strip_suffix(".git").unwrap_or(s);
    // Strip trailing slash
    s.trim_end_matches('/').to_string()
}
