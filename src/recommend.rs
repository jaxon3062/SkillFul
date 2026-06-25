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

    if recommendations.is_empty() {
        recommendations.push(
            "No obvious changes recommended yet. Record more skill_end events to improve signal."
                .to_string(),
        );
    }

    Ok(recommendations)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::build_recommendations;
    use crate::db::SkillStatsRow;

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
        )
        .expect("recommendations");

        assert_eq!(
            recommendations,
            vec!["No obvious changes recommended yet. Record more skill_end events to improve signal.".to_string()]
        );
    }
}
