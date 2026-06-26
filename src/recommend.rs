use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;

use crate::{
    db::{SkillHistoryRow, SkillOverlapRow, SkillStatsRow},
    stats::UnusedSkillsReport,
};

pub fn build_recommendations(
    skill_stats: &[SkillStatsRow],
    defined_skills_path: &Path,
    observed_skills: &[String],
    skill_history: &[SkillHistoryRow],
    skill_overlap: &[SkillOverlapRow],
) -> Result<Vec<String>> {
    let mut recommendations = Vec::new();
    let declared_skills = declared_skills(defined_skills_path)?;
    let history_by_skill: HashMap<&str, &SkillHistoryRow> =
        skill_history.iter().map(|row| (row.skill.as_str(), row)).collect();

    let unused =
        UnusedSkillsReport::from_declared_and_observed(defined_skills_path, observed_skills)?;
    for skill in unused.names() {
        recommendations.push(format!(
            "Consider removing or demoting `{skill}`. Reason: declared but not observed in local traces."
        ));
    }

    for stat in skill_stats {
        if let Some(success_rate) = stat.success_rate {
            if stat.uses >= 1 && success_rate < 0.5 {
                recommendations.push(format!(
                    "Improve `{}`. Reason: low success rate ({:.2}) across {} completed runs.",
                    stat.skill, success_rate, stat.uses
                ));
            }
        }

        if stat.retries > 0 {
            recommendations.push(format!(
                "Reduce retries for `{}`. Reason: {} retry events recorded so far.",
                stat.skill, stat.retries
            ));
        }

        if let Some(success_rate) = stat.success_rate {
            if stat.uses <= 3 && success_rate >= 0.95 {
                recommendations.push(format!(
                    "Promote `{}`. Reason: strong success rate ({:.2}) with comparatively low usage.",
                    stat.skill, success_rate
                ));
            }
        }
    }

    let inactivity_cutoff = Utc::now() - Duration::days(30);
    for history in skill_history {
        if !declared_skills.contains(history.skill.as_str()) {
            continue;
        }

        let Ok(last_seen) = DateTime::parse_from_rfc3339(&history.last_seen) else {
            continue;
        };
        if last_seen.with_timezone(&Utc) < inactivity_cutoff {
            recommendations.push(format!(
                "Consider demoting `{}`. Reason: no completed runs recorded since {}.",
                history.skill,
                last_seen.format("%Y-%m-%d")
            ));
        }
    }

    for overlap in skill_overlap {
        let Some(left_history) = history_by_skill.get(overlap.left_skill.as_str()) else {
            continue;
        };
        let Some(right_history) = history_by_skill.get(overlap.right_skill.as_str()) else {
            continue;
        };
        let baseline_sessions = left_history.sessions.min(right_history.sessions);
        if baseline_sessions < 2 {
            continue;
        }

        let overlap_ratio = overlap.shared_sessions as f64 / baseline_sessions as f64;
        if overlap.shared_sessions >= 2 && overlap_ratio >= 0.8 {
            recommendations.push(format!(
                "Consider merging or clarifying `{}` and `{}`. Reason: they co-occurred in {} of {} shared-session opportunities.",
                overlap.left_skill,
                overlap.right_skill,
                overlap.shared_sessions,
                baseline_sessions
            ));
        }
    }

    if recommendations.is_empty() {
        recommendations.push(
            "No obvious changes recommended yet. Record more skill_end events to improve signal."
                .to_string(),
        );
    }

    Ok(recommendations)
}

fn declared_skills(path: &Path) -> Result<HashSet<String>> {
    let contents = fs::read_to_string(path)?;
    let catalog: SkillsFile = toml::from_str(&contents)?;
    Ok(catalog.skill.into_iter().map(|skill| skill.name).collect())
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
    use std::fs;

    use chrono::{Duration, Utc};
    use tempfile::tempdir;

    use super::build_recommendations;
    use crate::db::{SkillHistoryRow, SkillOverlapRow, SkillStatsRow};

    #[test]
    fn missing_success_rate_does_not_trigger_improve_recommendation() {
        let temp = tempdir().expect("tempdir");
        let skills_path = temp.path().join("skills.toml");
        fs::write(
            &skills_path,
            "[[skill]]\nname = \"run_tests\"\ndescription = \"Run tests\"\ncategory = \"validation\"\n",
        )
        .expect("write skills");

        let recommendations = build_recommendations(
            &[SkillStatsRow {
                skill: "run_tests".to_string(),
                uses: 1,
                success_rate: None,
                avg_duration_ms: None,
                retries: 0,
                failures: 0,
            }],
            &skills_path,
            &["run_tests".to_string()],
            &[SkillHistoryRow {
                skill: "run_tests".to_string(),
                last_seen: Utc::now().to_rfc3339(),
                sessions: 1,
            }],
            &[],
        )
        .expect("recommendations");

        assert_eq!(
            recommendations,
            vec!["No obvious changes recommended yet. Record more skill_end events to improve signal.".to_string()]
        );
    }

    #[test]
    fn stale_skill_triggers_demote_recommendation() {
        let temp = tempdir().expect("tempdir");
        let skills_path = temp.path().join("skills.toml");
        fs::write(
            &skills_path,
            "[[skill]]\nname = \"run_tests\"\ndescription = \"Run tests\"\ncategory = \"validation\"\n",
        )
        .expect("write skills");

        let recommendations = build_recommendations(
            &[SkillStatsRow {
                skill: "run_tests".to_string(),
                uses: 4,
                success_rate: Some(1.0),
                avg_duration_ms: Some(100.0),
                retries: 0,
                failures: 0,
            }],
            &skills_path,
            &["run_tests".to_string()],
            &[SkillHistoryRow {
                skill: "run_tests".to_string(),
                last_seen: (Utc::now() - Duration::days(45)).to_rfc3339(),
                sessions: 4,
            }],
            &[],
        )
        .expect("recommendations");

        assert!(recommendations.iter().any(|line| {
            line.contains("Consider demoting `run_tests`")
                && line.contains("no completed runs recorded since")
        }));
    }

    #[test]
    fn overlapping_skills_trigger_merge_recommendation() {
        let temp = tempdir().expect("tempdir");
        let skills_path = temp.path().join("skills.toml");
        fs::write(
            &skills_path,
            "[[skill]]\nname = \"repo_search\"\ndescription = \"Search\"\ncategory = \"retrieval\"\n[[skill]]\nname = \"code_search\"\ndescription = \"Search code\"\ncategory = \"retrieval\"\n",
        )
        .expect("write skills");

        let recommendations = build_recommendations(
            &[],
            &skills_path,
            &["code_search".to_string(), "repo_search".to_string()],
            &[
                SkillHistoryRow {
                    skill: "repo_search".to_string(),
                    last_seen: Utc::now().to_rfc3339(),
                    sessions: 3,
                },
                SkillHistoryRow {
                    skill: "code_search".to_string(),
                    last_seen: Utc::now().to_rfc3339(),
                    sessions: 4,
                },
            ],
            &[SkillOverlapRow {
                left_skill: "code_search".to_string(),
                right_skill: "repo_search".to_string(),
                shared_sessions: 3,
            }],
        )
        .expect("recommendations");

        assert!(recommendations.iter().any(|line| {
            line.contains("Consider merging or clarifying `code_search` and `repo_search`")
        }));
    }
}
