use std::{fs, path::Path};

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;

use crate::db::{FailureRow, SkillStatsRow, TimelineRow};

pub struct StatsQuery {
    pub since: Option<String>,
    pub agent: Option<String>,
    pub skill: Option<String>,
}

impl StatsQuery {
    pub fn from_filter_args(
        since: Option<String>,
        agent: Option<String>,
        skill: Option<String>,
    ) -> Self {
        Self { since, agent, skill }
    }

    pub fn since_timestamp(&self) -> Result<Option<String>> {
        self.since.as_deref().map(parse_since).transpose()
    }
}

pub fn render_skill_stats(rows: &[SkillStatsRow]) -> String {
    let mut lines =
        vec!["skill          uses   success_rate   avg_ms   retries   failures".to_string()];

    if rows.is_empty() {
        lines.push("No matching skill_end events found.".to_string());
        return lines.join("\n");
    }

    for row in rows {
        lines.push(format!(
            "{:<14} {:<6} {:<14} {:<8} {:<8} {}",
            row.skill,
            row.uses,
            format_success_rate(row.success_rate),
            format!("{:.0}", row.avg_duration_ms.unwrap_or(0.0)),
            row.retries,
            row.failures
        ));
    }

    lines.join("\n")
}

pub fn render_timeline(rows: &[TimelineRow]) -> String {
    if rows.is_empty() {
        return "No events found.".to_string();
    }

    rows.iter()
        .map(|row| {
            let success =
                row.success.map(|value| if value { " success" } else { " failure" }).unwrap_or("");
            match &row.skill {
                Some(skill) => format!(
                    "{} {} {}{} [{}]",
                    short_timestamp(&row.timestamp),
                    row.event_type,
                    skill,
                    success,
                    row.session_id
                ),
                None => format!(
                    "{} {}{} [{}]",
                    short_timestamp(&row.timestamp),
                    row.event_type,
                    success,
                    row.session_id
                ),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn render_failures(rows: &[FailureRow]) -> String {
    if rows.is_empty() {
        return "No failures found.".to_string();
    }

    rows.iter()
        .map(|row| {
            format!(
                "{} session={} skill={} retries={} error={} output={}",
                short_timestamp(&row.timestamp),
                row.session_id,
                row.skill.as_deref().unwrap_or("-"),
                row.retry_count,
                row.error.as_deref().unwrap_or("-"),
                row.output_summary.as_deref().unwrap_or("-")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_since(raw: &str) -> Result<String> {
    let duration = if let Some(days) = raw.strip_suffix('d') {
        Duration::days(days.parse::<i64>().context("invalid day duration")?)
    } else if let Some(hours) = raw.strip_suffix('h') {
        Duration::hours(hours.parse::<i64>().context("invalid hour duration")?)
    } else if let Some(minutes) = raw.strip_suffix('m') {
        Duration::minutes(minutes.parse::<i64>().context("invalid minute duration")?)
    } else {
        anyhow::bail!("unsupported --since value: {raw}")
    };

    Ok((Utc::now() - duration).to_rfc3339())
}

fn short_timestamp(timestamp: &str) -> String {
    DateTime::parse_from_rfc3339(timestamp)
        .map(|dt| dt.format("%H:%M:%S").to_string())
        .unwrap_or_else(|_| timestamp.to_string())
}

fn format_success_rate(success_rate: Option<f64>) -> String {
    success_rate.map(|value| format!("{value:.2}")).unwrap_or_else(|| "unknown".to_string())
}

pub struct UnusedSkillsReport {
    names: Vec<String>,
}

impl UnusedSkillsReport {
    pub fn names(&self) -> &[String] {
        &self.names
    }

    pub fn render(&self) -> String {
        if self.names.is_empty() {
            return "No unused skills found.".to_string();
        }

        let mut lines = vec!["Unused skill candidates:".to_string()];
        for name in &self.names {
            lines.push(format!("- {name}"));
        }
        lines.join("\n")
    }
}

impl UnusedSkillsReport {
    pub fn from_declared_and_observed(path: &Path, observed: &[String]) -> Result<Self> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let catalog: SkillsFile = toml::from_str(&contents)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        let names = catalog
            .skill
            .into_iter()
            .map(|skill| skill.name)
            .filter(|name| !observed.iter().any(|seen| seen == name))
            .collect();
        Ok(Self { names })
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

#[cfg(test)]
mod tests {
    use crate::db::SkillStatsRow;

    use super::render_skill_stats;

    #[test]
    fn render_skill_stats_shows_unknown_for_missing_success() {
        let rendered = render_skill_stats(&[SkillStatsRow {
            skill: "run_tests".to_string(),
            uses: 1,
            success_rate: None,
            avg_duration_ms: Some(250.0),
            retries: 0,
            failures: 0,
        }]);

        assert!(rendered.contains("unknown"));
        assert!(!rendered.contains("0.00"));
    }
}
