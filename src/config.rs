use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const DEFAULT_CONFIG: &str = r#"[storage]
backend = "sqlite"
path = "~/.skilltrace/skilltrace.db"
jsonl_mirror = true

[agents.codex]
enabled = true
adapter = "wrapper"

[mcp]
enabled = true
transport = "stdio"

[privacy]
capture_raw_prompts = false
capture_raw_outputs = false
hash_sensitive_values = true

[skills]
definition_file = "skills.toml"
"#;

#[derive(Debug, Clone)]
pub struct StoragePaths {
    pub root: PathBuf,
}

impl StoragePaths {
    pub fn discover() -> Result<Self> {
        let home = dirs::home_dir().context("home directory is not available")?;
        Ok(Self { root: home.join(".skilltrace") })
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.root)
            .with_context(|| format!("failed to create {}", self.root.display()))
    }

    pub fn config_path(&self) -> PathBuf {
        self.root.join("config.toml")
    }

    pub fn database_path(&self) -> PathBuf {
        self.root.join("skilltrace.db")
    }

    pub fn jsonl_path(&self) -> PathBuf {
        self.root.join("events.jsonl")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub storage: StorageConfig,
    pub agents: AgentsConfig,
    pub mcp: McpConfig,
    pub privacy: PrivacyConfig,
    pub skills: SkillsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub backend: String,
    pub path: String,
    pub jsonl_mirror: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentsConfig {
    pub codex: AgentConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub enabled: bool,
    pub adapter: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    pub enabled: bool,
    pub transport: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyConfig {
    pub capture_raw_prompts: bool,
    pub capture_raw_outputs: bool,
    pub hash_sensitive_values: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsConfig {
    pub definition_file: String,
}

impl AppConfig {
    pub fn write_default_if_missing(paths: &StoragePaths) -> Result<()> {
        write_if_missing(&paths.config_path(), DEFAULT_CONFIG)?;
        write_if_missing(&paths.jsonl_path(), "")?;
        Ok(())
    }
}

fn write_if_missing(path: &Path, contents: &str) -> Result<()> {
    if path.exists() {
        return Ok(());
    }

    fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))
}
