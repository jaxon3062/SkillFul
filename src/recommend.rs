use std::path::Path;

use anyhow::Result;

use crate::{db::SkillStatsRow, stats::UnusedSkillsReport};

pub fn build_recommendations(
    skill_stats: &[SkillStatsRow],
    defined_skills_path: &Path,
    observed_skills: &[String],
) -> Result<Vec<String>> {
    let mut recommendations = Vec::new();

    let unused =
        UnusedSkillsReport::from_declared_and_observed(defined_skills_path, observed_skills)?;
    for skill in unused.names() {
        recommendations.push(format!(
            "Consider removing or demoting `{skill}`. Reason: declared but not observed in local traces."
        ));
    }

    for stat in skill_stats {
        if stat.uses >= 1 && stat.success_rate.unwrap_or(0.0) < 0.5 {
            recommendations.push(format!(
                "Improve `{}`. Reason: low success rate ({:.2}) across {} completed runs.",
                stat.skill,
                stat.success_rate.unwrap_or(0.0),
                stat.uses
            ));
        }

        if stat.retries > 0 {
            recommendations.push(format!(
                "Reduce retries for `{}`. Reason: {} retry events recorded so far.",
                stat.skill, stat.retries
            ));
        }

        if stat.uses <= 3 && stat.success_rate.unwrap_or(0.0) >= 0.95 {
            recommendations.push(format!(
                "Promote `{}`. Reason: strong success rate ({:.2}) with comparatively low usage.",
                stat.skill,
                stat.success_rate.unwrap_or(0.0)
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
