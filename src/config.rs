use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

pub const ENV_SESSION_ID: &str = "SKILLTRACE_SESSION_ID";
pub const ENV_AGENT: &str = "SKILLTRACE_AGENT";
pub const ENV_ADAPTER: &str = "SKILLTRACE_ADAPTER";

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

    pub fn state_path(&self) -> PathBuf {
        self.root.join("state.toml")
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

    pub fn load(paths: &StoragePaths) -> Result<Self> {
        let contents = fs::read_to_string(paths.config_path())
            .with_context(|| format!("failed to read {}", paths.config_path().display()))?;
        toml::from_str(&contents)
            .with_context(|| format!("failed to parse {}", paths.config_path().display()))
    }

    pub fn load_or_create(paths: &StoragePaths) -> Result<Self> {
        Self::write_default_if_missing(paths)?;
        Self::load(paths)
    }

    pub fn resolved_definition_file(&self, cwd: &Path) -> PathBuf {
        resolve_path(cwd, Path::new(&self.skills.definition_file))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeState {
    #[serde(default)]
    pub current_session_id: Option<String>,
    #[serde(default)]
    pub active_session_ids: Vec<String>,
}

impl RuntimeState {
    pub fn load(paths: &StoragePaths) -> Result<Self> {
        let path = paths.state_path();
        if !path.exists() {
            return Ok(Self { current_session_id: None, active_session_ids: Vec::new() });
        }

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        toml::from_str(&contents).with_context(|| format!("failed to parse {}", path.display()))
    }

    pub fn save(&self, paths: &StoragePaths) -> Result<()> {
        let contents = toml::to_string(self).context("failed to serialize runtime state")?;
        fs::write(paths.state_path(), contents)
            .with_context(|| format!("failed to write {}", paths.state_path().display()))
    }

    pub fn register_session(&mut self, session_id: &str) {
        if !self.active_session_ids.iter().any(|active| active == session_id) {
            self.active_session_ids.push(session_id.to_string());
        }
        self.current_session_id = Some(session_id.to_string());
    }

    pub fn unregister_session(&mut self, session_id: &str) {
        self.active_session_ids.retain(|active| active != session_id);
        self.current_session_id = match self.active_session_ids.as_slice() {
            [single] => Some(single.clone()),
            _ => None,
        };
    }

    pub fn preferred_session_id(&self) -> Option<String> {
        match self.active_session_ids.as_slice() {
            [single] => Some(single.clone()),
            [] => self.current_session_id.clone(),
            _ => None,
        }
    }

    pub fn preferred_session_id_with_env(&self, env_session_id: Option<&str>) -> Option<String> {
        env_session_id
            .filter(|session_id| !session_id.trim().is_empty())
            .map(str::to_string)
            .or_else(|| self.preferred_session_id())
    }

    pub fn preferred_session_id_from_environment(&self) -> Option<String> {
        let env_session_id = env::var(ENV_SESSION_ID).ok();
        self.preferred_session_id_with_env(env_session_id.as_deref())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub id: String,
    pub agent: String,
    pub adapter: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub cwd: Option<String>,
    pub repo: Option<String>,
    pub branch: Option<String>,
    pub metadata_json: Option<String>,
}

impl SessionRecord {
    pub fn new(agent: String, adapter: String, cwd: Option<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            agent,
            adapter,
            started_at: Utc::now().to_rfc3339(),
            ended_at: None,
            cwd: cwd.clone(),
            repo: cwd,
            branch: None,
            metadata_json: None,
        }
    }
}

fn write_if_missing(path: &Path, contents: &str) -> Result<()> {
    if path.exists() {
        return Ok(());
    }

    fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))
}

pub fn resolve_path(base_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() { path.to_path_buf() } else { base_dir.join(path) }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{AppConfig, RuntimeState, resolve_path};

    #[test]
    fn configured_definition_file_resolves_from_working_directory() {
        let config = AppConfig {
            storage: super::StorageConfig {
                backend: "sqlite".to_string(),
                path: "~/.skilltrace/skilltrace.db".to_string(),
                jsonl_mirror: true,
            },
            agents: super::AgentsConfig {
                codex: super::AgentConfig { enabled: true, adapter: "wrapper".to_string() },
            },
            mcp: super::McpConfig { enabled: true, transport: "stdio".to_string() },
            privacy: super::PrivacyConfig {
                capture_raw_prompts: false,
                capture_raw_outputs: false,
                hash_sensitive_values: true,
            },
            skills: super::SkillsConfig { definition_file: "config/skills.toml".to_string() },
        };

        assert_eq!(
            config.resolved_definition_file(Path::new("/repo")),
            Path::new("/repo/config/skills.toml")
        );
    }

    #[test]
    fn explicit_relative_paths_resolve_from_base_directory() {
        assert_eq!(
            resolve_path(Path::new("/repo"), Path::new("nested/skills.toml")),
            Path::new("/repo/nested/skills.toml")
        );
    }

    #[test]
    fn preferred_session_id_is_none_when_multiple_sessions_are_active() {
        let mut state = RuntimeState { current_session_id: None, active_session_ids: Vec::new() };
        state.register_session("session-a");
        state.register_session("session-b");

        assert_eq!(state.preferred_session_id(), None);
    }

    #[test]
    fn session_id_from_environment_takes_precedence_over_runtime_state() {
        let mut state = RuntimeState { current_session_id: None, active_session_ids: Vec::new() };
        state.register_session("session-runtime");

        assert_eq!(
            state.preferred_session_id_with_env(Some("session-env")),
            Some("session-env".to_string())
        );
    }
}
