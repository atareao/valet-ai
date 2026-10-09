//! Read-only handler for the closed skills catalog.
//!
//! The catalog lives in code (`crate::orchestrator::skills::catalog()`) and is
//! the single source of truth for the router and the evaluation harness. This
//! endpoint exposes it —plus the non-routable core set— so the interface never
//! duplicates the catalog in its own code.
//!
//! Besides the compiled defaults, the endpoint exposes the **effective** value
//! of every editable field and marks which ones come from `settings`: the
//! question, the two criteria and the per-skill threshold. It also exposes
//! whether each skill is **enabled** (`ROUTER_SKILL_<ID>_ENABLED`). A
//! question/criteria is overridden when its effective value differs from the
//! compiled default; a threshold is overridden when there is an explicit
//! per-skill override. The threshold precedence is the router's
//! ([`effective_threshold`]); the field precedence is [`effective_field`].
//! Neither rule is reimplemented here.

use std::collections::HashMap;

use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};

use crate::db::repos::settings::SettingsRepo;
use crate::orchestrator::skill_router::{
    effective_field, effective_threshold, read_router_config, SkillRouterConfig,
};
use crate::orchestrator::skills::{catalog, CORE_TOOLS};
use crate::AppState;

/// The effective configuration of one skill, as exposed by `GET /api/skills`.
///
/// `question` / `criteria_*` are the effective texts (the `settings` value when
/// it has content, otherwise the catalog's). `threshold` is the effective
/// threshold. `enabled` is false when the skill is in
/// [`SkillRouterConfig::disabled_skills`]. `overridden` names the fields that
/// come from `settings`: a question/criteria when its effective value differs
/// from the compiled default, a threshold when an explicit per-skill override
/// exists.
#[derive(Debug, Clone, PartialEq)]
pub struct SkillView {
    pub id: &'static str,
    pub prompt_key: &'static str,
    pub prompt_heading: &'static str,
    pub tools: &'static [&'static str],
    pub enabled: bool,
    pub question: String,
    pub criteria_true: String,
    pub criteria_false: String,
    pub threshold: f32,
    pub overridden: Vec<&'static str>,
}

impl SkillView {
    fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "prompt_key": self.prompt_key,
            "prompt_heading": self.prompt_heading,
            "tools": self.tools,
            "enabled": self.enabled,
            "question": self.question,
            "criteria_true": self.criteria_true,
            "criteria_false": self.criteria_false,
            "threshold": self.threshold,
            "overridden": self.overridden,
        })
    }
}

/// Compose the response for every skill in the catalog from the resolved router
/// configuration and the raw `settings` map.
///
/// Pure (no I/O): the caller reads `settings` once and hands it over, so the
/// composition can be tested without HTTP or a database.
///
/// A question/criteria is marked as overridden when its effective value differs
/// from the compiled default; a threshold is marked when `config` carries an
/// explicit per-skill override for that skill. `enabled` is false when the skill
/// is in [`SkillRouterConfig::disabled_skills`].
pub fn skill_views(
    config: &SkillRouterConfig,
    settings: &HashMap<String, String>,
) -> Vec<SkillView> {
    catalog()
        .iter()
        .map(|spec| {
            let id = spec.id.to_ascii_uppercase();

            let question = effective_field(
                settings
                    .get(&format!("SKILL_{id}_QUESTION"))
                    .map(String::as_str),
                spec.instructions,
            );
            let criteria_true = effective_field(
                settings
                    .get(&format!("SKILL_{id}_CRITERIA_TRUE"))
                    .map(String::as_str),
                spec.criteria_true,
            );
            let criteria_false = effective_field(
                settings
                    .get(&format!("SKILL_{id}_CRITERIA_FALSE"))
                    .map(String::as_str),
                spec.criteria_false,
            );
            let threshold = effective_threshold(config, spec);
            let enabled = !config.disabled_skills.contains(spec.id);

            let mut overridden: Vec<&'static str> = Vec::new();
            if question != spec.instructions {
                overridden.push("question");
            }
            if criteria_true != spec.criteria_true {
                overridden.push("criteria_true");
            }
            if criteria_false != spec.criteria_false {
                overridden.push("criteria_false");
            }
            // Only a genuine per-skill override counts: the compiled default is
            // not the yardstick here — presence in `threshold_overrides` is.
            if config.threshold_overrides.contains_key(spec.id) {
                overridden.push("threshold");
            }

            SkillView {
                id: spec.id,
                prompt_key: spec.prompt_key,
                prompt_heading: spec.prompt_heading,
                tools: spec.tools,
                enabled,
                question,
                criteria_true,
                criteria_false,
                threshold,
                overridden,
            }
        })
        .collect()
}

/// `GET /api/skills`
///
/// Serialises the closed catalog from [`catalog()`] (id, `prompt_key`,
/// `prompt_heading`, the tools each skill covers —prerequisites included—,
/// whether it is enabled, the effective question/criteria/threshold and which
/// fields are overridden) together with the always-exposed core set.
pub async fn list_skills(State(state): State<AppState>) -> Json<Value> {
    let config = read_router_config(&state.db).await;
    let settings = SettingsRepo::get_all(&state.db).await.unwrap_or_else(|e| {
        tracing::warn!(
            error = %e,
            "failed to read settings for /api/skills; falling back to the catalog"
        );
        HashMap::new()
    });

    let skills: Vec<Value> = skill_views(&config, &settings)
        .iter()
        .map(SkillView::to_json)
        .collect();

    Json(json!({
        "skills": skills,
        "core_tools": CORE_TOOLS,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_skill_without_an_override_uses_its_compiled_threshold() {
        let views = skill_views(&SkillRouterConfig::default(), &HashMap::new());

        for view in &views {
            let spec = catalog()
                .iter()
                .find(|spec| spec.id == view.id)
                .expect("every view comes from the catalog");
            assert!(
                (view.threshold - spec.threshold).abs() < 1e-6,
                "sin override `{}` usa su umbral compilado: {} != {}",
                view.id,
                view.threshold,
                spec.threshold
            );
            assert!(
                !view.overridden.contains(&"threshold"),
                "una skill sin override no se marca: {:?}",
                view.overridden
            );
        }
    }

    #[test]
    fn an_enabled_skill_is_reported_as_enabled_and_a_disabled_one_as_disabled() {
        let views = skill_views(&SkillRouterConfig::default(), &HashMap::new());
        assert!(
            views.iter().all(|v| v.enabled),
            "sin skills deshabilitadas todas figuran habilitadas"
        );

        let mut disabled_skills = std::collections::HashSet::new();
        disabled_skills.insert("widgets".to_string());
        let config = SkillRouterConfig {
            disabled_skills,
            ..Default::default()
        };

        let views = skill_views(&config, &HashMap::new());
        let widgets = views.iter().find(|v| v.id == "widgets").unwrap();
        assert!(!widgets.enabled, "una skill deshabilitada figura apagada");
        let agenda = views.iter().find(|v| v.id == "agenda").unwrap();
        assert!(agenda.enabled, "una skill habilitada sigue encendida");
    }

    #[test]
    fn a_per_skill_override_marks_only_that_skill_threshold() {
        let mut threshold_overrides = HashMap::new();
        threshold_overrides.insert("widgets".to_string(), 0.20f32);
        let config = SkillRouterConfig {
            threshold_overrides,
            ..Default::default()
        };

        let views = skill_views(&config, &HashMap::new());

        let widgets = views.iter().find(|v| v.id == "widgets").unwrap();
        assert!(
            widgets.overridden.contains(&"threshold"),
            "el override propio de widgets debe marcarse: {:?}",
            widgets.overridden
        );
        let agenda = views.iter().find(|v| v.id == "agenda").unwrap();
        assert!(
            !agenda.overridden.contains(&"threshold"),
            "una skill sin override no se marca: {:?}",
            agenda.overridden
        );
    }

    #[test]
    fn spaced_settings_values_are_declared_trimmed() {
        // The router sends the trimmed value; the API must declare exactly what
        // travels, not the raw settings string.
        let mut settings = HashMap::new();
        settings.insert(
            "SKILL_AGENDA_QUESTION".to_string(),
            "  ¿agenda?  ".to_string(),
        );
        settings.insert(
            "SKILL_WEB_CRITERIA_TRUE".to_string(),
            "\tweb true\t".to_string(),
        );

        let views = skill_views(&SkillRouterConfig::default(), &settings);

        let agenda = views.iter().find(|v| v.id == "agenda").unwrap();
        assert_eq!(agenda.question, "¿agenda?");
        assert!(
            agenda.overridden.contains(&"question"),
            "un valor recortado distinto del catálogo sigue marcado"
        );

        let web = views.iter().find(|v| v.id == "web").unwrap();
        assert_eq!(web.criteria_true, "web true");
    }
}
