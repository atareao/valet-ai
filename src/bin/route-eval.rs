//! `valet-route-eval` — evaluation harness for the per-turn skill router.
//!
//! It walks the historical turns whose **assistant** message records the tools
//! it actually used (`messages.tools_used`, format `"(N) tool::operation"`
//! joined by `", "`), runs the router on the preceding **user** message and
//! measures *coverage*: the share of turns whose every-used tool ends up in the
//! set the router would have exposed. It also reports per-skill activations,
//! classifier latency (p50/p95) and token/cost totals.
//!
//! It never writes to the database and, under `--dry-run`, never touches the
//! network.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use async_trait::async_trait;
use sqlx::{Row, SqlitePool};

use valet::config::Config;
use valet::llm::decisions::{
    DecisionsProvider, DecisionsRequest, DecisionsResponse, JevDecisionsConfig,
    JevDecisionsProvider,
};
use valet::llm::provider::LLMError;
use valet::orchestrator::skill_router::{
    effective_field, effective_threshold, exposed_tools, read_router_config, read_skill_criteria,
    SkillCriteria, SkillRouter, SkillRouterConfig,
};
use valet::orchestrator::skills::{catalog, skill_of_tool, Skill, CORE_TOOLS};

const DEFAULT_LIMIT: usize = 200;

/// Same per-turn character budget the orchestrator uses when it hands the
/// conversational state to the classifier (`ROUTER_HISTORY_TURN_MAX_CHARS`).
const HISTORY_TURN_MAX_CHARS: usize = 400;

const USAGE: &str = "\
valet-route-eval — measure the coverage of the per-turn skill router

USAGE:
    valet-route-eval [OPTIONS]

Walks the historical turns with a recorded `tools_used`, runs the router on the
preceding user message and reports how often every tool the turn really used
was part of the exposed set.

OPTIONS:
    --limit <N>        Maximum number of historical turns to evaluate (default 200).
    --threshold <F>    Override the router threshold (e.g. 0.4).
    --model <ID>       Override the decisions model (e.g. typesafe/jev-1.13).
    --db <URL>         Database URL; defaults to DATABASE_URL / config.
    --dry-run          Print the catalog and the pairing without any network call.
    -h, --help         Print this help and exit.
";

/// Parsed command-line arguments.
struct Args {
    limit: usize,
    threshold: Option<f32>,
    model: Option<String>,
    dry_run: bool,
    db: Option<String>,
}

/// One evaluable turn: an assistant message with `tools_used` and the user
/// message immediately before it.
struct Turn {
    assistant_id: String,
    user_message: String,
    used_tools: Vec<String>,
    /// Whether the recorded `tools_used` was corrupt: non-empty but unparseable.
    /// Such a turn is never covered (it would inflate the coverage).
    parse_failed: bool,
    /// The router's conversational state before this turn, built from the real
    /// `role: content` messages and reset whenever the pairing breaks.
    history: Vec<String>,
}

/// A message as read from the DB, before pairing.
struct RawMessage {
    id: String,
    role: String,
    content: String,
    tools_used: Option<String>,
}

/// A `role: content` entry of the classifier state, truncated exactly like
/// `orchestrator::agent` does before handing the history to the router.
fn format_history_entry(role: &str, content: &str) -> String {
    let turn = format!("{role}: {content}");
    turn.chars().take(HISTORY_TURN_MAX_CHARS).collect()
}

/// The catalog id of a skill, for the report.
fn skill_id(skill: Skill) -> &'static str {
    catalog()
        .iter()
        .find(|spec| spec.skill == skill)
        .map(|spec| spec.id)
        .unwrap_or("?")
}

/// Human-readable coverage of a tool that the exposed set lacked: the id of the
/// skill that would have covered it, `"core"` for a core tool (never routable)
/// or `"unknown"` for a name outside the catalog.
fn covering_skill(tool: &str) -> String {
    match skill_of_tool(tool) {
        Some(skill) => skill_id(skill).to_string(),
        None if CORE_TOOLS.contains(&tool) => "core".to_string(),
        None => "unknown".to_string(),
    }
}

/// `"yes"`/`"no"`, for the overridden flags of the configuration report.
fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

/// The effective-configuration report, one line per skill.
///
/// Pure (no I/O): it takes the resolved router configuration and the effective
/// criteria map and returns the report lines, so the harness can print the
/// configuration it actually used —the same in the dry run and in the measured
/// run— and a table test can assert the composition without the network.
///
/// Each line carries the skill id, its effective threshold and whether its
/// question and its criteria come from `settings` (they differ from the
/// compiled default). The threshold precedence and the field rule are the
/// router's ([`effective_threshold`] / [`effective_field`]); they are not
/// reimplemented here.
fn effective_config_rows(
    config: &SkillRouterConfig,
    criteria: &HashMap<String, SkillCriteria>,
) -> Vec<String> {
    catalog()
        .iter()
        .map(|spec| {
            let entry = criteria.get(spec.id);
            let question =
                effective_field(entry.map(|c| c.instructions.as_str()), spec.instructions);
            let criteria_true =
                effective_field(entry.map(|c| c.criteria_true.as_str()), spec.criteria_true);
            let criteria_false = effective_field(
                entry.map(|c| c.criteria_false.as_str()),
                spec.criteria_false,
            );

            let question_overridden = question != spec.instructions;
            let criteria_overridden =
                criteria_true != spec.criteria_true || criteria_false != spec.criteria_false;

            format!(
                "  {:<13} threshold={:.2}  question={:<3} criteria={:<3}",
                spec.id,
                effective_threshold(config, spec),
                yes_no(question_overridden),
                yes_no(criteria_overridden),
            )
        })
        .collect()
}

/// Why a turn is or is not covered.
#[derive(Debug, PartialEq)]
enum TurnOutcome {
    /// Every tool the turn used was exposed.
    Covered,
    /// These tools were used but not exposed.
    Missing(Vec<String>),
    /// The recorded `tools_used` was non-empty but parsed to nothing: corrupt
    /// data, never a covered turn.
    ParseFailed,
}

/// Classify a turn: corrupt `tools_used` is never covered, an empty missing
/// list is covered, anything else lists what was missing.
fn turn_outcome(used: &[String], exposed: &[String], parse_failed: bool) -> TurnOutcome {
    if parse_failed {
        return TurnOutcome::ParseFailed;
    }
    let missing = missing_tools(used, exposed);
    if missing.is_empty() {
        TurnOutcome::Covered
    } else {
        TurnOutcome::Missing(missing)
    }
}

/// One uncovered turn, as reported at the end of the run.
struct UncoveredTurn {
    assistant_id: String,
    used_tools: Vec<String>,
    missing_tools: Vec<String>,
    parse_failed: bool,
}

/// Split the real `messages.tools_used` payload into tool **names**.
///
/// Strips the optional `(N) ` occurrence counter and the `::operation` suffix,
/// keeps the first-seen order and de-duplicates names. An empty payload yields
/// an empty vector.
fn parse_tools_used(raw: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for part in raw.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        // Optional `(N) ` prefix: only present when the operation repeated.
        let without_count = match part.strip_prefix('(') {
            Some(rest) => match rest.find(')') {
                Some(end) => rest[end + 1..].trim(),
                None => part,
            },
            None => part,
        };
        let name = without_count.split("::").next().unwrap_or("").trim();
        if name.is_empty() {
            continue;
        }
        if !out.iter().any(|t| t.as_str() == name) {
            out.push(name.to_string());
        }
    }
    out
}

/// Tools the turn really used that are **not** in the exposed set, in the
/// order they were used.
fn missing_tools(used: &[String], exposed: &[String]) -> Vec<String> {
    used.iter()
        .filter(|tool| !exposed.iter().any(|e| e.as_str() == tool.as_str()))
        .cloned()
        .collect()
}

/// Covered share as a fraction in `[0, 1]`. `total == 0` is not an error: it
/// returns `0.0` instead of panicking.
fn coverage(covered: usize, total: usize) -> f32 {
    if total == 0 {
        return 0.0;
    }
    covered as f32 / total as f32
}

/// Nearest-rank percentile over an **ascending** slice. An empty slice yields
/// `0`. `p` is clamped to `[0, 1]`.
fn percentile(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let p = p.clamp(0.0, 1.0);
    let len = sorted.len();
    let rank = (p * len as f64).ceil() as usize;
    let index = rank.saturating_sub(1).min(len - 1);
    sorted[index]
}

/// Accumulated classifier usage.
#[derive(Debug, Default, Clone, Copy)]
struct UsageTotals {
    input_tokens: u64,
    output_tokens: u64,
    cost: f64,
}

/// `DecisionsProvider` decorator that forwards to the real client but records
/// the usage it reports, so the harness can aggregate tokens and cost without
/// the router exposing them.
struct RecordingDecisions {
    inner: Arc<dyn DecisionsProvider>,
    usage: Mutex<UsageTotals>,
}

impl RecordingDecisions {
    fn new(inner: Arc<dyn DecisionsProvider>) -> Self {
        Self {
            inner,
            usage: Mutex::new(UsageTotals::default()),
        }
    }

    fn totals(&self) -> UsageTotals {
        let guard = self
            .usage
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard
    }
}

#[async_trait]
impl DecisionsProvider for RecordingDecisions {
    async fn decide(&self, request: DecisionsRequest) -> Result<DecisionsResponse, LLMError> {
        let response = self.inner.decide(request).await?;
        {
            let mut usage = self
                .usage
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            usage.input_tokens += response.input_tokens;
            usage.output_tokens += response.output_tokens;
            usage.cost += response.cost;
        }
        Ok(response)
    }
}

/// Parse the raw arguments (excluding the program name).
fn parse_args(raw: &[String]) -> Result<Args, String> {
    let mut args = Args {
        limit: DEFAULT_LIMIT,
        threshold: None,
        model: None,
        dry_run: false,
        db: None,
    };

    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--limit" => {
                i += 1;
                let value = raw.get(i).ok_or("--limit requires a value")?;
                args.limit = value
                    .parse()
                    .map_err(|_| format!("invalid --limit: {value}"))?;
            }
            "--threshold" => {
                i += 1;
                let value = raw.get(i).ok_or("--threshold requires a value")?;
                args.threshold = Some(
                    value
                        .parse()
                        .map_err(|_| format!("invalid --threshold: {value}"))?,
                );
            }
            "--model" => {
                i += 1;
                let value = raw.get(i).ok_or("--model requires a value")?;
                args.model = Some(value.clone());
            }
            "--db" => {
                i += 1;
                let value = raw.get(i).ok_or("--db requires a value")?;
                args.db = Some(value.clone());
            }
            "--dry-run" => args.dry_run = true,
            other => return Err(format!("unknown argument '{other}'")),
        }
        i += 1;
    }

    Ok(args)
}

/// Whether a CLI-supplied threshold is usable: **finite** and within `[0, 1]`.
///
/// Production protects this in `read_threshold`; the harness must apply the
/// same guard, because `"NaN"`/`"inf"` parse successfully but would silence the
/// router (`prob >= NaN` is always false). An invalid `--threshold` is ignored
/// with a warning and the settings value is kept.
fn valid_threshold(threshold: f32) -> bool {
    threshold.is_finite() && (0.0..=1.0).contains(&threshold)
}

/// Whether `name` is in the enabled-tool set.
fn is_enabled_tool(enabled: &[String], name: &str) -> bool {
    enabled.iter().any(|e| e.as_str() == name)
}

/// Pair each assistant message that records `tools_used` with the user message
/// immediately before it, discarding pairs missing either side and building the
/// router's conversational state.
///
/// The state is the real `role: content` of every message since the start of
/// the current contiguous run (truncated exactly like production does),
/// mirroring `orchestrator::agent`. The run is reset whenever the pairing
/// breaks — when a turn's user message does not immediately follow the previous
/// turn's assistant message — so a context that never existed is not invented.
fn pair_turns(messages: &[RawMessage]) -> Vec<Turn> {
    let mut turns: Vec<Turn> = Vec::new();
    let mut run_start = 0usize;
    let mut previous_assistant: Option<usize> = None;

    for (index, message) in messages.iter().enumerate() {
        if message.role != "assistant" {
            continue;
        }
        let Some(raw) = message.tools_used.as_deref() else {
            continue;
        };
        if raw.trim().is_empty() {
            continue;
        }
        let Some(user_index) = (0..index).rev().find(|&j| messages[j].role == "user") else {
            continue;
        };

        // Contiguous only when this turn's user message immediately follows the
        // previous turn's assistant message.
        if let Some(previous) = previous_assistant {
            if user_index != previous + 1 {
                run_start = user_index;
            }
        }

        let used_tools = parse_tools_used(raw);
        let parse_failed = used_tools.is_empty();

        let history = messages[run_start..user_index]
            .iter()
            .map(|m| format_history_entry(&m.role, &m.content))
            .collect();

        turns.push(Turn {
            assistant_id: message.id.clone(),
            user_message: messages[user_index].content.clone(),
            used_tools,
            parse_failed,
            history,
        });

        previous_assistant = Some(index);
    }

    turns
}

/// Load every message in chronological order and pair the evaluable turns.
async fn load_turns(pool: &SqlitePool) -> Result<Vec<Turn>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, role, content, tools_used FROM messages \
         ORDER BY created_at ASC, id ASC",
    )
    .fetch_all(pool)
    .await?;

    let messages: Vec<RawMessage> = rows
        .iter()
        .map(|row| RawMessage {
            id: row.get("id"),
            role: row.get("role"),
            content: row.get("content"),
            tools_used: row.get("tools_used"),
        })
        .collect();

    Ok(pair_turns(&messages))
}

/// Keep at most the `limit` most recent turns.
fn cap_turns(mut turns: Vec<Turn>, limit: usize) -> Vec<Turn> {
    if turns.len() > limit {
        let drop = turns.len() - limit;
        turns.drain(0..drop);
    }
    turns
}

/// Resolve the production registry and return the enabled tool names, matching
/// the set the running application advertises.
async fn enabled_tools(pool: &SqlitePool) -> Vec<String> {
    let registry = valet::build_tool_registry(pool);
    if let Ok(disabled) = valet::db::repos::tools::ToolsRepo::disabled_names(pool).await {
        registry.set_disabled(disabled);
    }
    let mut names: Vec<String> = registry
        .definitions()
        .iter()
        .map(|def| def.name.clone())
        .collect();
    names.sort_unstable();
    names
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.iter().any(|a| a == "-h" || a == "--help") {
        print!("{USAGE}");
        return Ok(());
    }

    let args = match parse_args(&raw) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("error: {error}\n");
            eprint!("{USAGE}");
            std::process::exit(2);
        }
    };

    let db_url = args
        .db
        .clone()
        .unwrap_or_else(|| Config::from_env().database_url);
    let pool = valet::db::init_db(&db_url).await?;

    let enabled = enabled_tools(&pool).await;
    let turns = cap_turns(load_turns(&pool).await?, args.limit);

    // Resolve the effective configuration once, applying the CLI overrides, so
    // both the dry run and the measured run publish exactly what they use.
    let mut router_config: SkillRouterConfig = read_router_config(&pool).await;
    if let Some(threshold) = args.threshold {
        if valid_threshold(threshold) {
            router_config.threshold = threshold;
        } else {
            tracing::warn!(
                value = threshold,
                uses = router_config.threshold,
                "--threshold is not a finite value in [0, 1]; ignoring it and using the settings value"
            );
        }
    }
    if let Some(model) = &args.model {
        router_config.model = model.clone();
    }
    // The criteria the router will send: the live `settings` values, falling
    // back to the catalog. Read here so the measured run and the dry run agree.
    let criteria = read_skill_criteria(&pool).await;
    let config_rows = effective_config_rows(&router_config, &criteria);

    // Dry run: no classifier, no network. Print the catalog, the pairing and the
    // effective configuration that would be used.
    if args.dry_run {
        println!("DRY RUN — no network calls");
        println!();
        println!(
            "Catalog ({} skills, {} core tools):",
            catalog().len(),
            CORE_TOOLS.len()
        );
        for spec in catalog() {
            let enabled_in_skill: Vec<&str> = spec
                .tools
                .iter()
                .copied()
                .filter(|tool| is_enabled_tool(&enabled, tool))
                .collect();
            println!(
                "  - {:<13} key={:<24} tools={:?} enabled={:?}",
                spec.id, spec.prompt_key, spec.tools, enabled_in_skill
            );
        }
        println!("  core_tools = {CORE_TOOLS:?}");

        let routable = catalog()
            .iter()
            .filter(|spec| {
                spec.tools
                    .iter()
                    .any(|tool| is_enabled_tool(&enabled, tool))
            })
            .count();
        println!();
        println!("Enabled tools ({}): {enabled:?}", enabled.len());
        println!("Questions per turn: {routable} (one per routable skill)");
        println!(
            "Turns that would be evaluated: {} (limit={})",
            turns.len(),
            args.limit
        );
        println!();
        println!("Effective configuration (would be used):");
        println!("  global threshold = {:.2}", router_config.threshold);
        for row in &config_rows {
            println!("{row}");
        }
        return Ok(());
    }

    // Evaluation: force the router on (we are not measuring the switch). The
    // `--threshold` / `--model` overrides were already applied above.
    router_config.enabled = true;

    let Some(jev) =
        JevDecisionsConfig::from_env(router_config.model.clone(), router_config.timeout_ms)
    else {
        eprintln!(
            "error: OPENROUTER_API_KEY is not set — supply it or run with --dry-run \
             to inspect the catalog without the network"
        );
        std::process::exit(2);
    };

    let recording = Arc::new(RecordingDecisions::new(Arc::new(
        JevDecisionsProvider::new(jev),
    )));
    let provider: Option<Arc<dyn DecisionsProvider>> =
        Some(Arc::clone(&recording) as Arc<dyn DecisionsProvider>);
    // Build the router with the effective criteria, so the measured coverage
    // reflects the live configuration, not the compiled one.
    let router = SkillRouter::new(provider, router_config.clone()).with_criteria(criteria);

    let mut covered = 0usize;
    let mut latencies: Vec<u64> = Vec::with_capacity(turns.len());
    let mut activations: HashMap<Skill, usize> = HashMap::new();
    let mut uncovered: Vec<UncoveredTurn> = Vec::new();

    for turn in &turns {
        let started = Instant::now();
        let selection = router
            .select(&turn.user_message, &turn.history, &enabled)
            .await;
        latencies.push(started.elapsed().as_millis() as u64);

        for skill in &selection.skills {
            *activations.entry(*skill).or_insert(0) += 1;
        }

        let exposed = exposed_tools(&selection, &enabled);
        match turn_outcome(&turn.used_tools, &exposed, turn.parse_failed) {
            TurnOutcome::Covered => covered += 1,
            TurnOutcome::Missing(missing) => uncovered.push(UncoveredTurn {
                assistant_id: turn.assistant_id.clone(),
                used_tools: turn.used_tools.clone(),
                missing_tools: missing,
                parse_failed: false,
            }),
            TurnOutcome::ParseFailed => uncovered.push(UncoveredTurn {
                assistant_id: turn.assistant_id.clone(),
                used_tools: turn.used_tools.clone(),
                missing_tools: Vec::new(),
                parse_failed: true,
            }),
        }
    }

    let total = turns.len();
    latencies.sort_unstable();
    let p50 = percentile(&latencies, 0.50);
    let p95 = percentile(&latencies, 0.95);
    let usage = recording.totals();

    println!("== Skill routing evaluation ==");
    println!("Model:           {}", router_config.model);
    println!("Threshold (global): {:.3}", router_config.threshold);
    println!("Enabled tools:   {}", enabled.len());
    println!("Turns evaluated: {total}");
    println!(
        "Coverage:        {:.1}% ({covered}/{total})",
        coverage(covered, total) * 100.0
    );
    println!();
    println!("Effective configuration:");
    for row in &config_rows {
        println!("{row}");
    }
    println!();
    println!("Classifier latency: p50={p50} ms  p95={p95} ms");
    println!(
        "Tokens (input/output): {}/{}",
        usage.input_tokens, usage.output_tokens
    );
    println!("Cost: {:.6}", usage.cost);
    println!();
    println!("Activations per skill:");
    for spec in catalog() {
        let count = activations.get(&spec.skill).copied().unwrap_or(0);
        println!("  {:<13} {}", spec.id, count);
    }

    if uncovered.is_empty() {
        println!();
        println!("Uncovered turns: none");
    } else {
        println!();
        println!("Uncovered turns ({}):", uncovered.len());
        for turn in &uncovered {
            if turn.parse_failed {
                println!(
                    "  - {} tools_used={:?} (parse failure: counted as uncovered)",
                    turn.assistant_id, turn.used_tools
                );
                continue;
            }
            println!(
                "  - {} used={:?} missing={:?}",
                turn.assistant_id, turn.used_tools, turn.missing_tools
            );
            for tool in &turn.missing_tools {
                println!("      '{tool}' would need skill '{}'", covering_skill(tool));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tools_used_table() {
        let cases: &[(&str, &[&str])] = &[
            ("", &[]),
            ("   ", &[]),
            ("weather", &["weather"]),
            (
                "calendar::get_events, weather::get_weather",
                &["calendar", "weather"],
            ),
            (
                "(3) calendar::get_events, weather::get_weather",
                &["calendar", "weather"],
            ),
            (
                "(2) calendar::create_event, weather::get_weather, calendar::create_event",
                &["calendar", "weather"],
            ),
            (
                "(2) calendar::get_events, tasks::list_tasks",
                &["calendar", "tasks"],
            ),
            ("get_current_time", &["get_current_time"]),
            ("  weather  ,  (2) notes  ", &["weather", "notes"]),
        ];

        for (raw, expected) in cases {
            let got = parse_tools_used(raw);
            let expected: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
            assert_eq!(got, expected, "parse_tools_used({raw:?})");
        }
    }

    #[test]
    fn missing_tools_table() {
        let cases: &[(&[&str], &[&str], &[&str])] = &[
            (&["calendar"], &["calendar", "render_widget"], &[]),
            (&["weather"], &["render_widget"], &["weather"]),
            (
                &["calendar", "weather"],
                &["render_widget"],
                &["calendar", "weather"],
            ),
            (&[], &["render_widget"], &[]),
        ];

        for (used, exposed, expected) in cases {
            let used: Vec<String> = used.iter().map(|s| s.to_string()).collect();
            let exposed: Vec<String> = exposed.iter().map(|s| s.to_string()).collect();
            let expected: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
            assert_eq!(
                missing_tools(&used, &exposed),
                expected,
                "missing_tools({used:?}, {exposed:?})"
            );
        }
    }

    #[test]
    fn valid_threshold_accepts_only_finite_values_in_unit_range() {
        // The CLI `--threshold` must obey the same guard as `read_threshold`:
        // `NaN`/`inf` parse but would silence the router (`prob >= NaN` is
        // always false) and values outside `[0, 1]` are meaningless.
        for good in [0.0f32, 0.10, 0.5, 1.0] {
            assert!(valid_threshold(good), "{good} is a finite value in [0, 1]");
        }
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.1, 1.1] {
            assert!(!valid_threshold(bad), "{bad} must be rejected");
        }
    }

    #[test]
    fn coverage_table() {
        assert_eq!(coverage(0, 0), 0.0, "an empty run must not panic");
        assert_eq!(coverage(0, 4), 0.0);
        assert_eq!(coverage(3, 4), 0.75);
        assert_eq!(coverage(4, 4), 1.0);
    }

    #[test]
    fn percentile_table() {
        assert_eq!(percentile(&[], 0.5), 0, "an empty slice must not panic");

        let sorted = [10u64, 20, 30, 40];
        assert_eq!(percentile(&sorted, 0.0), 10);
        assert_eq!(percentile(&sorted, 0.5), 20);
        assert_eq!(percentile(&sorted, 0.95), 40);
        assert_eq!(percentile(&sorted, 1.0), 40);
        assert_eq!(percentile(&[7], 0.5), 7);
    }

    #[test]
    fn parse_args_reads_flags_and_rejects_unknown() {
        let raw: Vec<String> = [
            "--limit",
            "5",
            "--threshold",
            "0.4",
            "--model",
            "typesafe/jev-2.0",
            "--db",
            "test.db",
            "--dry-run",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        let args = parse_args(&raw).expect("valid arguments");
        assert_eq!(args.limit, 5);
        assert_eq!(args.threshold, Some(0.4));
        assert_eq!(args.model.as_deref(), Some("typesafe/jev-2.0"));
        assert_eq!(args.db.as_deref(), Some("test.db"));
        assert!(args.dry_run);

        assert!(parse_args(&["--nope".to_string()]).is_err());
        assert!(parse_args(&["--limit".to_string()]).is_err());
        assert!(parse_args(&["--limit".to_string(), "x".to_string()]).is_err());
    }

    #[test]
    fn cap_turns_keeps_the_most_recent() {
        let make = |id: &str| Turn {
            assistant_id: id.to_string(),
            user_message: String::new(),
            used_tools: Vec::new(),
            parse_failed: false,
            history: Vec::new(),
        };
        let turns = vec![make("a"), make("b"), make("c"), make("d")];

        let capped = cap_turns(turns, 2);
        assert_eq!(capped.len(), 2);
        assert_eq!(capped[0].assistant_id, "c");
        assert_eq!(capped[1].assistant_id, "d");
    }

    #[test]
    fn covering_skill_table() {
        let cases: &[(&str, &str)] = &[
            ("tasks", "pendientes"),
            ("reminders", "pendientes"),
            ("notes", "recuerdos"),
            ("unified_search", "recuerdos"),
            ("calendar", "agenda"),
            ("weather", "entorno"),
            ("geocode", "entorno"),
            ("web_search", "web"),
            ("render_widget", "widgets"),
            ("get_current_time", "core"),
            ("get_current_location", "core"),
            ("no_existe", "unknown"),
        ];

        for (tool, expected) in cases {
            assert_eq!(
                covering_skill(tool),
                *expected,
                "covering_skill({tool:?}) must name the skill that would cover it"
            );
        }
    }

    #[test]
    fn turn_outcome_table() {
        let s = |items: &[&str]| items.iter().map(|s| s.to_string()).collect::<Vec<_>>();

        assert_eq!(
            turn_outcome(&s(&["calendar"]), &s(&["calendar", "render_widget"]), false),
            TurnOutcome::Covered
        );
        assert_eq!(
            turn_outcome(&s(&["weather"]), &s(&["render_widget"]), false),
            TurnOutcome::Missing(s(&["weather"]))
        );
        assert_eq!(
            turn_outcome(&s(&[]), &s(&["render_widget"]), false),
            TurnOutcome::Covered,
            "a genuinely empty tools_used is covered"
        );
        assert_eq!(
            turn_outcome(&s(&[]), &s(&["render_widget"]), true),
            TurnOutcome::ParseFailed,
            "a corrupt non-empty tools_used must never count as covered"
        );
    }

    /// Build a raw DB message for the pairing tests.
    fn raw(role: &str, content: &str, tools_used: Option<&str>, id: &str) -> RawMessage {
        RawMessage {
            id: id.to_string(),
            role: role.to_string(),
            content: content.to_string(),
            tools_used: tools_used.map(str::to_string),
        }
    }

    #[test]
    fn format_history_entry_truncates_like_production() {
        assert_eq!(format_history_entry("assistant", "hola"), "assistant: hola");

        let long = "x".repeat(500);
        let entry = format_history_entry("user", &long);
        assert_eq!(
            entry.chars().count(),
            HISTORY_TURN_MAX_CHARS,
            "the entry must be truncated to the production budget"
        );
        assert!(entry.starts_with("user: xxx"));
    }

    #[test]
    fn pair_turns_builds_history_with_real_roles() {
        let messages = vec![
            raw("user", "hola", None, "u1"),
            raw("assistant", "¿qué tal?", None, "a1"),
            raw("user", "pon una tarea", None, "u2"),
            raw("assistant", "hecho", Some("tasks::create"), "a2"),
        ];

        let turns = pair_turns(&messages);

        assert_eq!(turns.len(), 1, "only the tool-bearing assistant pairs");
        assert_eq!(turns[0].assistant_id, "a2");
        assert_eq!(turns[0].user_message, "pon una tarea");
        assert_eq!(
            turns[0].history,
            vec!["user: hola", "assistant: ¿qué tal?"],
            "the history carries the real roles and contents, not just the user message"
        );
    }

    #[test]
    fn pair_turns_resets_history_when_pairing_breaks() {
        let messages = vec![
            raw("user", "uno", None, "u1"),
            raw("assistant", "r1", Some("tasks::list"), "a1"),
            raw("other", "sistema", None, "x1"), // breaks contiguity
            raw("user", "dos", None, "u2"),
            raw("assistant", "r2", Some("tasks::list"), "a2"),
        ];

        let turns = pair_turns(&messages);

        assert_eq!(turns.len(), 2);
        assert_eq!(
            turns[1].history,
            Vec::<String>::new(),
            "the broken pairing resets the accumulated state"
        );
    }

    #[test]
    fn pair_turns_keeps_history_when_pairing_holds() {
        let messages = vec![
            raw("user", "uno", None, "u1"),
            raw("assistant", "r1", Some("tasks::list"), "a1"),
            raw("user", "dos", None, "u2"),
            raw("assistant", "r2", Some("tasks::list"), "a2"),
        ];

        let turns = pair_turns(&messages);

        assert_eq!(turns.len(), 2);
        assert_eq!(
            turns[1].history,
            vec!["user: uno", "assistant: r1"],
            "a contiguous turn keeps the previous turn as context"
        );
    }

    #[test]
    fn pair_turns_flags_corrupt_tools_used() {
        let messages = vec![
            raw("user", "hola", None, "u1"),
            raw("assistant", "r", Some("(3)"), "a1"), // non-empty but parses to nothing
        ];

        let turns = pair_turns(&messages);

        assert_eq!(turns.len(), 1);
        assert!(turns[0].used_tools.is_empty());
        assert!(
            turns[0].parse_failed,
            "a non-empty tools_used that parses to empty must be flagged"
        );
        assert_eq!(
            turn_outcome(
                &turns[0].used_tools,
                &["render_widget".to_string()],
                turns[0].parse_failed
            ),
            TurnOutcome::ParseFailed
        );
    }

    // ─── Effective-configuration report (skill-router-tuning 7.1/7.2) ───────

    /// The measured router config as production ships it: global 0.10 plus the
    /// seeded widget override at 0.20.
    fn measured_config() -> SkillRouterConfig {
        let mut threshold_overrides = HashMap::new();
        threshold_overrides.insert("widgets".to_string(), 0.20f32);
        SkillRouterConfig {
            threshold: 0.10,
            threshold_overrides,
            ..Default::default()
        }
    }

    /// The report line for a skill, matched by its fixed-width id column.
    fn row_for<'a>(rows: &'a [String], id: &str) -> &'a str {
        let needle = format!("{id:<13}");
        rows.iter()
            .find(|row| row.contains(&needle))
            .map(String::as_str)
            .unwrap_or_else(|| panic!("no row for skill {id}: {rows:?}"))
    }

    #[test]
    fn effective_config_rows_default_to_the_catalog() {
        let rows = effective_config_rows(&measured_config(), &HashMap::new());

        assert_eq!(rows.len(), catalog().len(), "one row per skill: {rows:?}");
        for id in [
            "agenda",
            "pendientes",
            "recuerdos",
            "entorno",
            "web",
            "widgets",
        ] {
            let row = row_for(&rows, id);
            assert!(
                row.contains("question=no"),
                "unset question must not be marked: {row}"
            );
            assert!(
                row.contains("criteria=no"),
                "unset criteria must not be marked: {row}"
            );
        }
        assert!(
            row_for(&rows, "agenda").contains("threshold=0.10"),
            "domain skills use the global 0.10"
        );
        assert!(
            row_for(&rows, "widgets").contains("threshold=0.20"),
            "the seeded widget override is published"
        );
    }

    #[test]
    fn effective_config_rows_mark_overridden_question_and_criteria_per_skill() {
        let mut criteria = HashMap::new();
        criteria.insert(
            "agenda".to_string(),
            SkillCriteria {
                instructions: "¿agenda sobrescrita?".to_string(),
                criteria_true: String::new(),
                criteria_false: String::new(),
            },
        );
        criteria.insert(
            "entorno".to_string(),
            SkillCriteria {
                instructions: String::new(),
                criteria_true: "true sobrescrito".to_string(),
                criteria_false: "false sobrescrito".to_string(),
            },
        );

        let rows = effective_config_rows(&measured_config(), &criteria);

        let agenda = row_for(&rows, "agenda");
        assert!(
            agenda.contains("question=yes"),
            "overridden question: {agenda}"
        );
        assert!(
            agenda.contains("criteria=no"),
            "untouched criteria: {agenda}"
        );

        let entorno = row_for(&rows, "entorno");
        assert!(
            entorno.contains("question=no"),
            "untouched question: {entorno}"
        );
        assert!(
            entorno.contains("criteria=yes"),
            "overridden criteria: {entorno}"
        );

        let web = row_for(&rows, "web");
        assert!(
            web.contains("question=no"),
            "a skill without overrides: {web}"
        );
        assert!(
            web.contains("criteria=no"),
            "a skill without overrides: {web}"
        );
    }

    #[test]
    fn effective_config_rows_publish_the_threshold_override() {
        let mut threshold_overrides = HashMap::new();
        threshold_overrides.insert("widgets".to_string(), 0.55f32);
        let config = SkillRouterConfig {
            threshold: 0.10,
            threshold_overrides,
            ..Default::default()
        };

        let rows = effective_config_rows(&config, &HashMap::new());
        assert!(
            row_for(&rows, "widgets").contains("threshold=0.55"),
            "the per-skill override is the effective threshold"
        );
        assert!(
            row_for(&rows, "agenda").contains("threshold=0.10"),
            "skills without override keep the global"
        );
    }
}
