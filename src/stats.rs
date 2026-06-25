use std::{fs, path::Path};

use anyhow::{Context, Result};
use serde::Deserialize;

pub struct StatsQuery {
    since: Option<String>,
    agent: Option<String>,
    skill: Option<String>,
}

impl StatsQuery {
    pub fn from_filter_args(
        since: Option<String>,
        agent: Option<String>,
        skill: Option<String>,
    ) -> Self {
        Self { since, agent, skill }
    }

    pub fn render_table(&self) -> String {
        let scope =
            format!("scope since={:?} agent={:?} skill={:?}", self.since, self.agent, self.skill);
        format!(
            "skill          uses   success_rate   avg_ms   retries   failures\n{scope}\nrun_tests      0      0.00           0        0         0"
        )
    }
}

pub struct UnusedSkillsReport {
    names: Vec<String>,
}

impl UnusedSkillsReport {
    pub fn from_file(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let catalog: SkillsFile = toml::from_str(&contents)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        let names = catalog.skill.into_iter().map(|skill| skill.name).collect();
        Ok(Self { names })
    }

    pub fn render(&self) -> String {
        if self.names.is_empty() {
            return "No declared skills found.".to_string();
        }

        let mut lines = vec!["Unused skill candidates:".to_string()];
        for name in &self.names {
            lines.push(format!("- {name}"));
        }
        lines.join("\n")
    }
}

#[derive(Debug, Deserialize)]
struct SkillsFile {
    skill: Vec<SkillDefinition>,
}

#[derive(Debug, Deserialize)]
struct SkillDefinition {
    name: String,
}
