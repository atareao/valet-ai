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
use valet::llm::provider::{LLMError, ToolDef};
use valet::orchestrator::skill_router::{
    effective_field, effective_threshold, exposed_tools, read_router_config, read_skill_criteria,
    Selection, SelectionSource, SkillCriteria, SkillRouter, SkillRouterConfig,
};
use valet::orchestrator::skills::{catalog, skill_of_tool, Skill, CORE_TOOLS};
use valet::token_estimate::estimate_json_tokens;

const DEFAULT_LIMIT: usize = 200;

/// Default token budget of the classifier history window, matching
/// `settings.max_window_tokens` (and `orchestrator::agent`).
const MAX_WINDOW_TOKENS_DEFAULT: usize = 10000;

/// Same per-turn character budget the orchestrator uses when it hands the
/// conversational state to the classifier (`ROUTER_HISTORY_TURN_MAX_CHARS`).
const HISTORY_TURN_MAX_CHARS: usize = 400;

/// The proximity bands the report aggregates near misses over: how many
/// classified-but-unselected skills were left within each **distance to their
/// effective threshold** (`threshold - prob`). Cumulative and closed on the
/// upper bound (`0 <= d <= band`).
const NEAR_MISS_BANDS: &[f32] = &[0.01, 0.02, 0.05, 0.10];

const USAGE: &str = "\
valet-route-eval — measure the coverage of the per-turn skill router

USAGE:
    valet-route-eval [OPTIONS]

Walks the historical turns with a recorded `tools_used`, runs the router on the
preceding user message and reports how often every tool the turn really used
was part of the exposed set.

OPTIONS:
    --limit <N>        Maximum number of historical turns to evaluate (default 200).
    --repeat <N>       Repeat the whole sweep N times and report the variance (default 1).
    --threshold <F>    Override the router threshold (e.g. 0.4).
    --model <ID>       Override the decisions model (e.g. typesafe/jev-1.13).
    --overrides <PATH> Read thresholds (global + per skill) and criteria from a JSON file
                       (precedence: CLI > file > settings).
    --db <URL>         Database URL; defaults to DATABASE_URL / config.
    --dry-run          Print the catalog and the pairing without any network call.
    -h, --help         Print this help and exit.
";

/// Parsed command-line arguments.
struct Args {
    limit: usize,
    threshold: Option<f32>,
    model: Option<String>,
    /// Path to a JSON overrides file (`--overrides`), if any.
    overrides: Option<String>,
    /// Number of times to repeat the whole sweep (`--repeat`).
    repeat: usize,
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
    /// The router's conversational state before this turn: the messages inside
    /// the token-budget window that precedes the user message, formatted as
    /// `role: content` with the effective (collapsed) content.
    history: Vec<String>,
}

/// A message as read from the DB, before pairing.
struct RawMessage {
    id: String,
    role: String,
    content: String,
    /// The collapsed (summarised) version of `content`, when it exists.
    collapsed_content: Option<String>,
    /// Token cost of the raw `content`.
    tokens_count: usize,
    /// Token cost of the collapsed content, when it exists.
    collapsed_tokens_count: usize,
    tools_used: Option<String>,
}

impl RawMessage {
    /// Contenido efectivo: el colapsado cuando existe, el crudo en caso contrario.
    fn effective_content(&self) -> &str {
        match &self.collapsed_content {
            Some(collapsed) => collapsed,
            None => &self.content,
        }
    }

    /// Coste en tokens efectivo: el colapsado cuando existe, el crudo en caso contrario.
    ///
    /// La condición de producción es la **presencia** del colapsado
    /// (`collapsed_content IS NOT NULL`), no un valor distinto de cero.
    fn effective_tokens(&self) -> usize {
        match &self.collapsed_content {
            Some(_) => self.collapsed_tokens_count,
            None => self.tokens_count,
        }
    }
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

/// Whether `id` is a catalog skill id — the only ids a per-skill override can
/// target. `"global"` is deliberately **not** a skill id.
fn is_known_skill_id(id: &str) -> bool {
    catalog().iter().any(|spec| spec.id == id)
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
    /// One diagnostic per missing tool that maps to a routable skill.
    diagnostics: Vec<MissingTool>,
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
        overrides: None,
        repeat: 1,
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
            "--repeat" => {
                i += 1;
                let value = raw.get(i).ok_or("--repeat requires a value")?;
                let repeat: usize = value
                    .parse()
                    .map_err(|_| format!("invalid --repeat: {value}"))?;
                if repeat == 0 {
                    return Err("--repeat must be at least 1".to_string());
                }
                args.repeat = repeat;
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
            "--overrides" => {
                i += 1;
                let value = raw.get(i).ok_or("--overrides requires a value")?;
                args.overrides = Some(value.clone());
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

/// Índice del primer mensaje que entra en la ventana: se cuenta hacia atrás desde
/// el mensaje inmediatamente anterior al del usuario, acumulando `effective_tokens`.
/// Se incluye mientras el acumulado **no supere** el presupuesto y se para en el
/// primero que lo supere (producción usa `SUM(...) <= budget` sobre los más
/// recientes, así que un mensaje que no cabe excluye también a todos los más
/// antiguos). Si el más reciente por sí solo ya supera el presupuesto, la ventana
/// queda vacía (`user_index`). Un presupuesto `0` **no** vacía la ventana: igual
/// que producción (`cumulative <= 0`), entran los mensajes más recientes de coste
/// `0` hasta el primero que aporte un token.
fn window_start(messages: &[RawMessage], user_index: usize, budget_tokens: usize) -> usize {
    let mut accumulated = 0usize;
    let mut start = user_index;
    for index in (0..user_index).rev() {
        accumulated = accumulated.saturating_add(messages[index].effective_tokens());
        if accumulated > budget_tokens {
            break;
        }
        start = index;
    }
    start
}

/// Pair each assistant message that records `tools_used` with the user message
/// immediately before it, discarding pairs missing either side and building the
/// router's conversational state.
///
/// The state is the window production would load: the messages before the user
/// message that fit in `budget_tokens`, counted backwards from the message just
/// before the turn, each formatted as `role: content` with the effective
/// (collapsed) content and truncated exactly like `orchestrator::agent`. The
/// window is fixed by the token budget, **not** by pairing contiguity, so a
/// broken pairing never drops context that production keeps.
///
/// Sampling bias: a turn here is an **assistant** message with `tools_used`,
/// while production calls the router **once per user message**. If a single user
/// message ever produced several tool-bearing assistants, the harness would
/// count the same router input several times and the coverage would be
/// over-weighted towards those turns. Measured on the real database: 66
/// tool-bearing assistants ↔ 66 distinct user messages, zero users with more
/// than one tool-bearing assistant ⇒ factor 1,00x, the bias does not trigger
/// today. This is deliberately not changed (the delta defines the turn as the
/// tool-bearing assistant message); but if a user message ever accumulates
/// several tool-bearing assistants, coverage would be biased **low** and we
/// would have to decide then whether to group by user message.
fn pair_turns(messages: &[RawMessage], budget_tokens: usize) -> Vec<Turn> {
    let mut turns: Vec<Turn> = Vec::new();

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

        let used_tools = parse_tools_used(raw);
        let parse_failed = used_tools.is_empty();

        let start = window_start(messages, user_index, budget_tokens);
        let history = messages[start..user_index]
            .iter()
            .map(|m| format_history_entry(&m.role, m.effective_content()))
            .collect();

        turns.push(Turn {
            assistant_id: message.id.clone(),
            user_message: messages[user_index].content.clone(),
            used_tools,
            parse_failed,
            history,
        });
    }

    turns
}

/// Load every message in chronological order and pair the evaluable turns.
///
/// The tie-break is `rowid` (SQLite's insertion order, which is how the table
/// physically returns equal `created_at`s), **not** `id`: `id` is a random v4
/// UUID, so ordering by it would invent an arbitrary order among rows that share
/// a `created_at` — an order production does not have, because it orders the
/// cumulative window by `created_at DESC` and the result by `created_at ASC`
/// with no tie-break. This is preventive fidelity: in the real database there
/// are 256 messages with 256 distinct `created_at`s and zero ties, so today this
/// changes no figure. `messages` is a normal rowid table (`id TEXT PRIMARY KEY`
/// does not make it `WITHOUT ROWID`).
async fn load_turns(pool: &SqlitePool, budget_tokens: usize) -> Result<Vec<Turn>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, role, content, tools_used, collapsed_content, tokens_count, \
         collapsed_tokens_count FROM messages \
         ORDER BY created_at ASC, rowid ASC",
    )
    .fetch_all(pool)
    .await?;

    let messages: Vec<RawMessage> = rows
        .iter()
        .map(|row| RawMessage {
            id: row.get("id"),
            role: row.get("role"),
            content: row.get("content"),
            collapsed_content: row.get("collapsed_content"),
            tokens_count: row.get::<i64, _>("tokens_count").max(0) as usize,
            collapsed_tokens_count: row.get::<i64, _>("collapsed_tokens_count").max(0) as usize,
            tools_used: row.get("tools_used"),
        })
        .collect();

    Ok(pair_turns(&messages, budget_tokens))
}

/// `settings.max_window_tokens`, with the production default (`10000`) when it
/// is absent or unparseable.
async fn max_window_tokens(pool: &SqlitePool) -> usize {
    valet::db::repos::settings::SettingsRepo::get(pool, "max_window_tokens")
        .await
        .ok()
        .flatten()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(MAX_WINDOW_TOKENS_DEFAULT)
}

/// Keep at most the `limit` most recent turns.
fn cap_turns(mut turns: Vec<Turn>, limit: usize) -> Vec<Turn> {
    if turns.len() > limit {
        let drop = turns.len() - limit;
        turns.drain(0..drop);
    }
    turns
}

/// (bytes, tokens estimados) del bloque de definiciones serializado tal como
/// viaja en la petición. Un fallo de serialización no tumba el arnés: `(0, 0)`.
fn tool_block_size(defs: &[ToolDef]) -> (usize, usize) {
    match serde_json::to_string(defs) {
        Ok(json) => (json.len(), estimate_json_tokens(&json)),
        Err(_) => (0, 0),
    }
}

/// Porcentaje ahorrado de `part` frente a `whole`: `(1 - part/whole) * 100`.
/// `whole == 0` ⇒ `0.0` (referencia vacía: no hay base sobre la que medir el
/// ahorro y el cociente sería `NaN`). Un `part == 0` no es un caso especial:
/// significa que se ha eliminado todo, o sea un ahorro del **100 %**.
fn savings_pct(part: usize, whole: usize) -> f32 {
    if whole == 0 {
        return 0.0;
    }
    (1.0 - part as f32 / whole as f32) * 100.0
}

/// Acumulado de la palanca a lo largo del barrido.
#[derive(Debug, Default, Clone, Copy)]
struct Leverage {
    turns: usize,
    exposed_tools: usize,
    exposed_bytes: usize,
    exposed_tokens: usize,
    full_bytes: usize,
    full_tokens: usize,
}

/// Líneas del informe con la palanca: herramientas por turno y ahorro en bytes y
/// tokens estimados, además de los tamaños absolutos expuesto y completo.
fn leverage_rows(l: &Leverage) -> Vec<String> {
    let tools_per_turn = if l.turns == 0 {
        0.0
    } else {
        l.exposed_tools as f32 / l.turns as f32
    };

    vec![
        format!(
            "  tools per turn:     {tools_per_turn:.2} (exposed {} over {} turns)",
            l.exposed_tools, l.turns
        ),
        format!(
            "  exposed block:      {} bytes, {} tokens est.",
            l.exposed_bytes, l.exposed_tokens
        ),
        format!(
            "  full enabled block: {} bytes, {} tokens est.",
            l.full_bytes, l.full_tokens
        ),
        format!(
            "  saving:             {:.1}% bytes, {:.1}% tokens est.",
            savings_pct(l.exposed_bytes, l.full_bytes),
            savings_pct(l.exposed_tokens, l.full_tokens)
        ),
    ]
}

/// Líneas del resumen de varianza (mín./media/máx.); vacío si solo hubo una medición.
fn variance_rows(coverages: &[f32]) -> Vec<String> {
    if coverages.len() < 2 {
        return Vec::new();
    }
    let min = coverages.iter().copied().reduce(f32::min).unwrap_or(0.0);
    let max = coverages.iter().copied().reduce(f32::max).unwrap_or(0.0);
    let mean = coverages.iter().sum::<f32>() / coverages.len() as f32;
    vec![format!(
        "  min={:.1}%  mean={:.1}%  max={:.1}%",
        min * 100.0,
        mean * 100.0,
        max * 100.0
    )]
}

/// Sufijo de las líneas del informe que acumulan sobre las repeticiones: vacío
/// para un solo barrido (así `--repeat 1` se imprime exactamente igual que antes,
/// sin ruido) o `" (all repetitions)"` cuando el barrido se repite.
fn all_repetitions_label(repeat: usize) -> &'static str {
    if repeat > 1 {
        " (all repetitions)"
    } else {
        ""
    }
}

/// La línea `Turns evaluated`. Con más de una repetición el total es la suma de
/// los barridos, así que la línea explicita cuántas repeticiones de cuántos turnos
/// lo produjeron.
fn turns_evaluated_line(total: usize, repeat: usize, per_repeat: usize) -> String {
    if repeat > 1 {
        format!("Turns evaluated: {total} ({repeat} repetitions of {per_repeat})")
    } else {
        format!("Turns evaluated: {total}")
    }
}

/// La línea `Coverage`. Con más de una repetición la cobertura es la **agrupada**
/// (que además coincide con la media de las coberturas por repetición).
fn coverage_line(covered: usize, total: usize, repeat: usize) -> String {
    let pct = coverage(covered, total) * 100.0;
    if repeat > 1 {
        format!("Coverage (pooled over {repeat} repetitions): {pct:.1}% ({covered}/{total})")
    } else {
        format!("Coverage: {pct:.1}% ({covered}/{total})")
    }
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

    // Build the production registry once, with the persisted disables applied,
    // and derive the enabled names from it. The same registry measures the
    // exposed-vs-full definition blocks, so there is a single source of truth.
    let registry = valet::build_tool_registry(&pool);
    if let Ok(disabled) = valet::db::repos::tools::ToolsRepo::disabled_names(&pool).await {
        registry.set_disabled(disabled);
    }
    let mut enabled: Vec<String> = registry
        .definitions()
        .iter()
        .map(|def| def.name.clone())
        .collect();
    enabled.sort_unstable();
    let budget_tokens = max_window_tokens(&pool).await;
    let turns = cap_turns(load_turns(&pool, budget_tokens).await?, args.limit);

    // Resolve the effective configuration once, applying the CLI overrides, so
    // both the dry run and the measured run publish exactly what they use.
    let mut router_config: SkillRouterConfig = read_router_config(&pool).await;
    if let Some(model) = &args.model {
        router_config.model = model.clone();
    }

    // Overrides file (if declared): an absent or unreadable file is a loud, hard
    // failure. Measuring with the wrong configuration silently is forbidden.
    let overrides = match &args.overrides {
        Some(path) => match load_overrides(path) {
            Ok(overrides) => Some(overrides),
            Err(error) => {
                eprintln!("error: {error}");
                std::process::exit(2);
            }
        },
        None => None,
    };

    // Global threshold precedence: CLI > file > settings. The `--threshold`
    // guard is kept: an invalid CLI value is discarded, never silences the
    // router. The effective value is resolved by precedence, so an invalid CLI
    // threshold may still end up being the file's global, not the settings one.
    let cli_threshold = match args.threshold {
        Some(threshold) if valid_threshold(threshold) => Some(threshold),
        Some(threshold) => {
            tracing::warn!(
                value = threshold,
                "--threshold is not a finite value in [0, 1]; discarding it and resolving the \
                 effective value by precedence (CLI > file > settings)"
            );
            None
        }
        None => None,
    };
    let file_global_threshold = overrides
        .as_ref()
        .and_then(|overrides| overrides.thresholds.get("global").copied());
    router_config.threshold = resolve_threshold(
        cli_threshold,
        file_global_threshold,
        router_config.threshold,
    );

    // The criteria the router will send: the live `settings` values, falling
    // back to the catalog, with the file overrides on top. Read here so the
    // measured run and the dry run agree.
    let mut criteria = read_skill_criteria(&pool).await;
    if let Some(overrides) = &overrides {
        // Per-skill thresholds and criteria: the file sits above settings. The
        // merge returns new maps, so the settings maps are never mutated, and
        // the `"global"` threshold is deliberately not part of the per-skill
        // map (it was already resolved into `router_config.threshold` above).
        router_config.threshold_overrides =
            effective_thresholds(&router_config.threshold_overrides, overrides);
        criteria = effective_criteria(&criteria, overrides);
    }

    let config_rows = effective_config_rows(&router_config, &criteria);

    // Attribute every effective override to its origin: the CLI `--threshold`
    // (only when it survived the guard) and `--model`, then the file. The CLI
    // sits above the file, so it is listed first.
    let mut cli_thresholds: HashMap<String, f32> = HashMap::new();
    if let Some(threshold) = cli_threshold {
        cli_thresholds.insert("global".to_string(), threshold);
    }
    let no_criteria: HashMap<String, SkillCriteria> = HashMap::new();
    let mut overrides_rows = Vec::new();
    if !cli_thresholds.is_empty() || args.model.is_some() {
        overrides_rows.extend(overrides_report(
            "cli",
            &cli_thresholds,
            &no_criteria,
            args.model.as_deref(),
        ));
    }
    if let Some(overrides) = &overrides {
        overrides_rows.extend(overrides_report(
            "file",
            &overrides.thresholds,
            &overrides.criteria,
            None,
        ));
    }

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
        println!("Window tokens (would be used): {budget_tokens}");
        println!();
        println!("Effective configuration (would be used):");
        println!("  global threshold = {:.2}", router_config.threshold);
        for row in &config_rows {
            println!("{row}");
        }
        if !overrides_rows.is_empty() {
            println!();
            println!("Active overrides (would be applied):");
            for row in &overrides_rows {
                println!("{row}");
            }
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
    let mut total_turns = 0usize;
    let mut latencies: Vec<u64> = Vec::new();
    let mut activations: HashMap<Skill, usize> = HashMap::new();
    let mut first_uncovered: Vec<UncoveredTurn> = Vec::new();
    let mut coverages: Vec<f32> = Vec::with_capacity(args.repeat);
    let mut leverage = Leverage::default();
    let mut all_diags: Vec<MissingTool> = Vec::new();

    for repetition in 0..args.repeat {
        let mut rep_covered = 0usize;
        let mut rep_uncovered: Vec<UncoveredTurn> = Vec::new();

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
            let exposed_refs: Vec<&str> = exposed.iter().map(String::as_str).collect();

            // Instrumentación de la palanca: el bloque expuesto frente al
            // conjunto completo habilitado, medido sobre las definiciones que
            // viajan en la petición. El completo se suma una vez por turno para
            // que el cociente sea comparable.
            let (exposed_bytes, exposed_tokens) =
                tool_block_size(&registry.definitions_for(&exposed_refs));
            let (full_bytes, full_tokens) = tool_block_size(&registry.definitions());
            leverage.turns += 1;
            leverage.exposed_tools += exposed.len();
            leverage.exposed_bytes += exposed_bytes;
            leverage.exposed_tokens += exposed_tokens;
            leverage.full_bytes += full_bytes;
            leverage.full_tokens += full_tokens;

            match turn_outcome(&turn.used_tools, &exposed, turn.parse_failed) {
                TurnOutcome::Covered => rep_covered += 1,
                TurnOutcome::Missing(missing) => {
                    let diagnostics =
                        missing_diagnostics(&turn.used_tools, &selection, &enabled, &router_config);
                    rep_uncovered.push(UncoveredTurn {
                        assistant_id: turn.assistant_id.clone(),
                        used_tools: turn.used_tools.clone(),
                        missing_tools: missing,
                        diagnostics,
                        parse_failed: false,
                    });
                }
                TurnOutcome::ParseFailed => rep_uncovered.push(UncoveredTurn {
                    assistant_id: turn.assistant_id.clone(),
                    used_tools: turn.used_tools.clone(),
                    missing_tools: Vec::new(),
                    diagnostics: Vec::new(),
                    parse_failed: true,
                }),
            }
        }

        covered += rep_covered;
        total_turns += turns.len();
        coverages.push(coverage(rep_covered, turns.len()));
        all_diags.extend(
            rep_uncovered
                .iter()
                .flat_map(|turn| turn.diagnostics.iter().cloned()),
        );
        if repetition == 0 {
            first_uncovered = rep_uncovered;
        }
    }

    latencies.sort_unstable();
    let p50 = percentile(&latencies, 0.50);
    let p95 = percentile(&latencies, 0.95);
    let usage = recording.totals();

    println!("== Skill routing evaluation ==");
    println!("Model:           {}", router_config.model);
    println!("Threshold (global): {:.3}", router_config.threshold);
    println!("Enabled tools:   {}", enabled.len());
    println!(
        "{}",
        turns_evaluated_line(total_turns, args.repeat, turns.len())
    );
    println!("{}", coverage_line(covered, total_turns, args.repeat));

    if args.repeat > 1 {
        println!();
        println!("Coverage per repetition:");
        for (index, value) in coverages.iter().enumerate() {
            println!("  run {}: {:.1}%", index + 1, value * 100.0);
        }
        println!();
        println!("Variance across repetitions:");
        for row in variance_rows(&coverages) {
            println!("{row}");
        }
    }

    println!();
    println!("Effective configuration:");
    for row in &config_rows {
        println!("{row}");
    }
    if !overrides_rows.is_empty() {
        println!();
        println!("Active overrides:");
        for row in &overrides_rows {
            println!("{row}");
        }
    }
    println!();
    println!(
        "Classifier latency{}: p50={p50} ms  p95={p95} ms",
        all_repetitions_label(args.repeat)
    );
    println!(
        "Tokens (input/output){}: {}/{}",
        all_repetitions_label(args.repeat),
        usage.input_tokens,
        usage.output_tokens
    );
    println!(
        "Cost{}: {:.6}",
        all_repetitions_label(args.repeat),
        usage.cost
    );
    println!();
    println!("Leverage (exposed vs full enabled set):");
    for row in leverage_rows(&leverage) {
        println!("{row}");
    }
    println!();
    println!("Activations per skill (mean per turn):");
    for spec in catalog() {
        let count = activations.get(&spec.skill).copied().unwrap_or(0);
        let mean = if total_turns == 0 {
            0.0
        } else {
            count as f32 / total_turns as f32
        };
        println!("  {:<13} {mean:.2}", spec.id);
    }

    if first_uncovered.is_empty() {
        println!();
        println!("Uncovered turns: none");
    } else {
        println!();
        println!(
            "Uncovered turns ({}) — first repetition:",
            first_uncovered.len()
        );
        for turn in &first_uncovered {
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
            for diag in &turn.diagnostics {
                let probability = match diag.probability {
                    Some(probability) => format!("{probability:.2}"),
                    None => "none".to_string(),
                };
                println!(
                    "      '{}' → skill={} prob={} threshold={:.2} source={:?}",
                    diag.tool,
                    skill_id(diag.skill),
                    probability,
                    diag.threshold,
                    diag.source,
                );
            }
        }
    }

    if !all_diags.is_empty() {
        println!();
        println!(
            "Proximity to threshold (failures within each distance){}:",
            all_repetitions_label(args.repeat)
        );
        for (band, count) in near_miss_bands(&all_diags, NEAR_MISS_BANDS) {
            println!("  <= {band:.2}: {count}");
        }
    }

    Ok(())
}

// ─── Diagnóstico de los turnos no cubiertos (route-eval-campaign-support) ───

/// Un fallo explicado: una herramienta usada que no se expuso, la skill que la
/// habría cubierto, la probabilidad que el clasificador le dio, el umbral
/// efectivo con el que se comparó y de dónde salió la selección.
#[derive(Debug, Clone, PartialEq)]
struct MissingTool {
    tool: String,
    skill: Skill,
    /// Probabilidad de «sí» de la skill, o `None` si el clasificador no la
    /// puntuó (fallo abierto o skill no preguntada).
    probability: Option<f32>,
    /// Umbral efectivo con el que se comparó esa probabilidad.
    threshold: f32,
    /// Fuente de la selección (`Router` / `Disabled` / `NoRoutableSkills` / `Error`).
    source: SelectionSource,
}

/// Por cada herramienta usada que NO figure en el conjunto expuesto
/// (`exposed_tools(selection, enabled)`), resuelve el diagnóstico: la skill que
/// la habría cubierto (`skill_of_tool`), su probabilidad en la `Selection`
/// (`None` si no está) y el umbral efectivo del config, más la fuente.
///
/// `enabled` es necesario porque el conjunto expuesto es
/// `exposed_tools(selection, enabled)`.
fn missing_diagnostics(
    used: &[String],
    selection: &Selection,
    enabled: &[String],
    config: &SkillRouterConfig,
) -> Vec<MissingTool> {
    let exposed = exposed_tools(selection, enabled);
    used.iter()
        .filter(|tool| !exposed.iter().any(|e| e.as_str() == tool.as_str()))
        .filter_map(|tool| {
            let skill = skill_of_tool(tool)?;
            let spec = catalog().iter().find(|spec| spec.skill == skill)?;
            let probability = selection
                .probabilities
                .iter()
                .find(|(selected, _)| *selected == skill)
                .map(|(_, probability)| *probability);
            Some(MissingTool {
                tool: tool.clone(),
                skill,
                probability,
                threshold: effective_threshold(config, spec),
                source: selection.source.clone(),
            })
        })
        .collect()
}

/// Proximidad al umbral en bandas: para cada banda `b`, cuántos diagnósticos
/// tienen probabilidad presente y quedan a una **distancia al umbral efectivo**
/// de a lo sumo `b` (`d = threshold - prob` con `d` en `[0, b]`, extremo superior
/// inclusivo, coherente con la etiqueta `<=`). Acumulativo: una banda mayor nunca
/// cuenta menos que una menor.
///
/// La distancia se mide contra el `threshold` **efectivo** guardado en cada
/// diagnóstico, no contra la probabilidad cruda: dos fallos con la misma
/// probabilidad pueden quedar a distinta distancia si sus umbrales difieren.
/// Los diagnósticos sin probabilidad se ignoran.
///
/// El borde se compara con una tolerancia `1e-6`: en `f32`, `0.10 - 0.08` da
/// `0.020000003`, que un `<= 0.02` estricto dejaría fuera aunque la etiqueta
/// promete `<=`. La tolerancia no rompe la monotonicidad acumulativa (sigue
/// creciendo con la banda).
fn near_miss_bands(diags: &[MissingTool], bands: &[f32]) -> Vec<(f32, usize)> {
    const EDGE_EPSILON: f32 = 1e-6;
    bands
        .iter()
        .map(|&band| {
            let count = diags
                .iter()
                .filter(|diag| {
                    diag.probability.is_some_and(|probability| {
                        let distance = diag.threshold - probability;
                        distance >= 0.0 && distance <= band + EDGE_EPSILON
                    })
                })
                .count();
            (band, count)
        })
        .collect()
}

// ─── Overrides desde fichero (route-eval-campaign-support) ──────────────────

/// Overrides declarados en un fichero: umbrales (con la clave `"global"` y los
/// ids de skill como claves) y criterios por id de skill.
#[derive(Debug, Clone, Default, PartialEq)]
struct Overrides {
    thresholds: HashMap<String, f32>,
    criteria: HashMap<String, SkillCriteria>,
}

/// Parsea el JSON de overrides. Devuelve `Err` con un mensaje explícito si el
/// JSON no es válido, no tiene la forma esperada o trae un umbral fuera de rango
/// (no finito o fuera de `[0, 1]`): un umbral inválido silenciaría (nunca/todas
/// las veces) al router y mediría en falso. Las claves desconocidas solo avisan
/// con `warn!`, nunca fallan.
fn parse_overrides(json: &str) -> Result<Overrides, String> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("invalid overrides JSON: {e}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| "overrides JSON must be an object".to_string())?;

    let mut overrides = Overrides::default();

    for key in object.keys() {
        if key != "thresholds" && key != "criteria" {
            tracing::warn!(
                key = %key,
                "unknown overrides top-level key; ignoring it"
            );
        }
    }

    if let Some(thresholds) = object.get("thresholds") {
        let map = thresholds
            .as_object()
            .ok_or_else(|| "overrides.thresholds must be an object".to_string())?;
        for (id, value) in map {
            let number = value
                .as_f64()
                .ok_or_else(|| format!("override threshold '{id}' must be a number"))?;
            let threshold = number as f32;
            if !valid_threshold(threshold) {
                return Err(format!("threshold '{id}' out of range: {value}"));
            }
            // `global` is the file-global threshold, not a skill id.
            if id != "global" && !is_known_skill_id(id) {
                tracing::warn!(
                    skill = %id,
                    "override threshold for an unknown skill id; it matches no catalog skill"
                );
            }
            overrides.thresholds.insert(id.clone(), threshold);
        }
    }

    if let Some(criteria) = object.get("criteria") {
        let map = criteria
            .as_object()
            .ok_or_else(|| "overrides.criteria must be an object".to_string())?;
        for (id, value) in map {
            let entry = value
                .as_object()
                .ok_or_else(|| format!("override criteria '{id}' must be an object"))?;
            if !is_known_skill_id(id) {
                tracing::warn!(
                    skill = %id,
                    "override criteria for an unknown skill id; it matches no catalog skill"
                );
            }
            // A missing field falls back to the empty string so that
            // `with_criteria` applies the catalog default, exactly as an
            // absent `settings` value would.
            let field = |key: &str| -> Result<String, String> {
                match entry.get(key) {
                    None => Ok(String::new()),
                    Some(value) => value
                        .as_str()
                        .map(str::to_string)
                        .ok_or_else(|| format!("override criteria '{id}.{key}' must be a string")),
                }
            };
            overrides.criteria.insert(
                id.clone(),
                SkillCriteria {
                    instructions: field("instructions")?,
                    criteria_true: field("criteria_true")?,
                    criteria_false: field("criteria_false")?,
                },
            );
        }
    }

    Ok(overrides)
}

/// Lee y parsea el fichero de overrides. Un fichero declarado y ausente o
/// ilegible devuelve `Err` explícito: nunca se mide en silencio con la
/// configuración equivocada.
fn load_overrides(path: &str) -> Result<Overrides, String> {
    let json = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read overrides file '{path}': {e}"))?;
    parse_overrides(&json)
}

/// Precedencia del umbral global: CLI > fichero > settings.
fn resolve_threshold(cli: Option<f32>, file: Option<f32>, settings: f32) -> f32 {
    cli.or(file).unwrap_or(settings)
}

/// Umbrales efectivos: los de `settings` con los del fichero por encima, **sin
/// mutar** el mapa de `settings`. La clave `"global"` nunca entra en el
/// resultado: el umbral global se resuelve aparte (`resolve_threshold` en
/// `router_config.threshold`), así que `threshold_overrides` solo lleva overrides
/// por skill.
fn effective_thresholds(
    settings: &HashMap<String, f32>,
    overrides: &Overrides,
) -> HashMap<String, f32> {
    let mut merged = settings.clone();
    merged.remove("global");
    for (id, value) in &overrides.thresholds {
        if id == "global" {
            continue;
        }
        merged.insert(id.clone(), *value);
    }
    merged
}

/// Criterios efectivos: los de `settings` con los del fichero por encima, **sin
/// mutar** el mapa de `settings`.
fn effective_criteria(
    settings: &HashMap<String, SkillCriteria>,
    overrides: &Overrides,
) -> HashMap<String, SkillCriteria> {
    let mut merged = settings.clone();
    for (id, value) in &overrides.criteria {
        merged.insert(id.clone(), value.clone());
    }
    merged
}

/// Bloque del informe con los overrides activos y su origen: cada línea declara
/// un modelo, un umbral o unos criterios y de dónde vino el valor (`origin`).
fn overrides_report(
    origin: &str,
    thresholds: &HashMap<String, f32>,
    criteria: &HashMap<String, SkillCriteria>,
    model: Option<&str>,
) -> Vec<String> {
    let mut rows = Vec::new();

    if let Some(model) = model {
        rows.push(format!("  model = {model} (from {origin})"));
    }

    let mut threshold_ids: Vec<&String> = thresholds.keys().collect();
    threshold_ids.sort();
    for id in threshold_ids {
        let value = thresholds.get(id).copied().unwrap_or_default();
        rows.push(format!("  threshold[{id}] = {value:.2} (from {origin})"));
    }

    let mut criteria_ids: Vec<&String> = criteria.keys().collect();
    criteria_ids.sort();
    for id in criteria_ids {
        rows.push(format!("  criteria[{id}] overridden (from {origin})"));
    }

    rows
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

    /// Build a raw DB message for the pairing tests, with the collapsed fields
    /// left at their no-collapse defaults (cost 0, no summary).
    fn raw(role: &str, content: &str, tools_used: Option<&str>, id: &str) -> RawMessage {
        raw_full(role, content, tools_used, id, None, 0, 0)
    }

    /// Build a raw DB message with every field under test spelled out.
    fn raw_full(
        role: &str,
        content: &str,
        tools_used: Option<&str>,
        id: &str,
        collapsed_content: Option<&str>,
        tokens_count: usize,
        collapsed_tokens_count: usize,
    ) -> RawMessage {
        RawMessage {
            id: id.to_string(),
            role: role.to_string(),
            content: content.to_string(),
            collapsed_content: collapsed_content.map(str::to_string),
            tokens_count,
            collapsed_tokens_count,
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

        let turns = pair_turns(&messages, MAX_WINDOW_TOKENS_DEFAULT);

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
    fn history_window_does_not_reset_when_pairing_breaks() {
        // Inverse subject of the old `pair_turns_resets_history_when_pairing_breaks`:
        // production fixes the window by token budget, not by pairing contiguity,
        // so a broken pairing must NOT drop the context that production keeps.
        let messages = vec![
            raw("user", "uno", None, "u1"),
            raw("assistant", "r1", Some("tasks::list"), "a1"),
            raw("other", "sistema", None, "x1"), // breaks contiguity
            raw("user", "dos", None, "u2"),
            raw("assistant", "r2", Some("tasks::list"), "a2"),
        ];

        let turns = pair_turns(&messages, MAX_WINDOW_TOKENS_DEFAULT);

        assert_eq!(turns.len(), 2);
        assert_eq!(
            turns[1].history,
            vec!["user: uno", "assistant: r1", "other: sistema"],
            "a broken pairing must not reset the window: the context production \
             would have is kept"
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

        let turns = pair_turns(&messages, MAX_WINDOW_TOKENS_DEFAULT);

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

        let turns = pair_turns(&messages, MAX_WINDOW_TOKENS_DEFAULT);

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

    // ─── Fidelidad del estado del clasificador (route-eval-fidelity) ────────

    #[test]
    fn effective_content_prefers_the_collapsed_version() {
        let collapsed = raw_full(
            "assistant",
            "texto larguísimo",
            None,
            "a1",
            Some("resumen"),
            500,
            80,
        );
        assert_eq!(
            collapsed.effective_content(),
            "resumen",
            "the collapsed content is the effective one"
        );

        let plain = raw("assistant", "texto crudo", None, "a2");
        assert_eq!(
            plain.effective_content(),
            "texto crudo",
            "without a collapsed version the raw content is effective"
        );
    }

    #[test]
    fn effective_tokens_prefers_the_collapsed_count() {
        let collapsed = raw_full(
            "assistant",
            "texto larguísimo",
            None,
            "a1",
            Some("resumen"),
            500,
            80,
        );
        assert_eq!(
            collapsed.effective_tokens(),
            80,
            "the collapsed token count is the effective cost"
        );

        let plain = raw_full("assistant", "texto crudo", None, "a2", None, 500, 0);
        assert_eq!(
            plain.effective_tokens(),
            500,
            "without a collapse the raw token count is effective"
        );
    }

    #[test]
    fn history_uses_the_effective_content() {
        let messages = vec![
            raw("user", "hola", None, "u1"),
            raw_full(
                "assistant",
                "texto larguísimo sin colapsar",
                None,
                "a1",
                Some("resumen"),
                500,
                80,
            ),
            raw("user", "pon una tarea", None, "u2"),
            raw("assistant", "hecho", Some("tasks::create"), "a2"),
        ];

        let turns = pair_turns(&messages, MAX_WINDOW_TOKENS_DEFAULT);

        assert_eq!(turns.len(), 1);
        assert_eq!(
            turns[0].history,
            vec!["user: hola", "assistant: resumen"],
            "the history must carry the collapsed content, not the raw one"
        );
    }

    #[test]
    fn history_window_is_fixed_by_the_token_budget() {
        // `u1` is oversized: a tight budget must drop it and a generous one must
        // keep it. Both sides are checked so neither can regress silently.
        let messages = vec![
            raw_full("user", "antiguo", None, "u1", None, 5_000, 5_000),
            raw_full("assistant", "r1", None, "a1", None, 100, 100),
            raw_full("user", "reciente", None, "u2", None, 100, 100),
            raw_full(
                "assistant",
                "respuesta",
                Some("tasks::list"),
                "a2",
                None,
                100,
                100,
            ),
        ];

        let tight = pair_turns(&messages, 200);
        assert_eq!(tight.len(), 1);
        assert_eq!(
            tight[0].history,
            vec!["assistant: r1"],
            "a tight budget must exclude the old oversized message"
        );

        let generous = pair_turns(&messages, MAX_WINDOW_TOKENS_DEFAULT);
        assert_eq!(generous.len(), 1);
        assert_eq!(
            generous[0].history,
            vec!["user: antiguo", "assistant: r1"],
            "a generous budget must include the old message"
        );
    }

    #[test]
    fn zero_budget_keeps_zero_cost_messages_like_production() {
        // Production counts `cumulative <= budget` over the most recent rows, so
        // a budget of 0 still includes the trailing run of zero-cost messages and
        // stops at the first one that contributes a token.
        let messages = vec![
            raw_full("user", "viejo", None, "u1", None, 0, 0),
            raw_full("assistant", "r1", None, "a1", None, 0, 0),
            raw_full("user", "reciente", None, "u2", None, 50, 50),
            raw_full(
                "assistant",
                "respuesta",
                Some("tasks::list"),
                "a2",
                None,
                10,
                10,
            ),
        ];

        let turns = pair_turns(&messages, 0);

        assert_eq!(turns.len(), 1);
        assert_eq!(
            turns[0].history,
            vec!["user: viejo", "assistant: r1"],
            "a zero budget must keep the zero-cost messages up to the first real token"
        );
    }

    // ─── Instrumentación y repetición (route-eval-fidelity) ─────────────────

    #[test]
    fn tool_block_size_measures_the_serialized_definitions() {
        let defs = vec![
            ToolDef {
                name: "weather".to_string(),
                description: "Get the weather".to_string(),
                parameters: serde_json::json!({"type": "object", "properties": {}}),
            },
            ToolDef {
                name: "tasks".to_string(),
                description: "Manage tasks".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {"id": {"type": "string"}}
                }),
            },
        ];

        let json = serde_json::to_string(&defs).expect("ToolDef must serialize");
        let expected = (json.len(), estimate_json_tokens(&json));

        assert_eq!(
            tool_block_size(&defs),
            expected,
            "the block size must come from the serialized definitions the request carries"
        );

        // Size alone is tautological (same object both sides). Also pin the
        // **wire shape** the OpenRouter request carries, so the test would fail
        // if `ToolDef`'s custom `Serialize` stopped emitting the wrapped form.
        let value: serde_json::Value =
            serde_json::from_str(&json).expect("the serialized block must be valid JSON");
        let array = value
            .as_array()
            .expect("the definition block must serialize to a JSON array");
        assert_eq!(array.len(), defs.len());

        for (entry, def) in array.iter().zip(&defs) {
            assert_eq!(
                entry.get("type").and_then(|v| v.as_str()),
                Some("function"),
                "each entry must carry the OpenRouter `type:\"function\"` wrapper: {entry}"
            );
            let function = entry
                .get("function")
                .and_then(|v| v.as_object())
                .expect("each entry must carry the `function` object");
            assert_eq!(
                function.get("name").and_then(|v| v.as_str()),
                Some(def.name.as_str()),
                "the `function` object must carry the tool name"
            );
            assert!(
                function
                    .get("description")
                    .and_then(|v| v.as_str())
                    .is_some(),
                "the `function` object must carry `description`: {function:?}"
            );
            assert!(
                function.get("parameters").is_some(),
                "the `function` object must carry `parameters`: {function:?}"
            );
        }
    }

    #[test]
    fn savings_pct_table() {
        assert_eq!(savings_pct(250, 1000), 75.0);
        assert_eq!(
            savings_pct(0, 1000),
            100.0,
            "an empty exposed block means everything was cut: a 100% saving, not 0"
        );
        assert_eq!(
            savings_pct(1000, 1000),
            0.0,
            "no saving when nothing is cut"
        );
        let degenerate = savings_pct(5, 0);
        assert_eq!(degenerate, 0.0, "a zero reference must yield 0.0");
        assert!(
            !degenerate.is_nan(),
            "a zero reference must never yield NaN"
        );
    }

    #[test]
    fn leverage_rows_publish_the_leverage() {
        let l = Leverage {
            turns: 4,
            exposed_tools: 10,
            exposed_bytes: 250,
            exposed_tokens: 40,
            full_bytes: 1000,
            full_tokens: 200,
        };

        let rows = leverage_rows(&l);
        assert!(!rows.is_empty(), "the leverage must be published");
        let joined = rows.join("\n");

        assert!(
            joined.contains("2.5"),
            "tools per turn must be published: {joined}"
        );
        assert!(
            joined.contains("75.0"),
            "the byte saving must be published: {joined}"
        );
        assert!(
            joined.contains("80.0"),
            "the token saving must be published: {joined}"
        );
    }

    #[test]
    fn variance_rows_table() {
        let rows = variance_rows(&[0.92, 0.95, 0.92]);
        assert!(
            !rows.is_empty(),
            "three measurements have a variance to publish"
        );
        let joined = rows.join("\n");
        for expected in ["92", "93", "95"] {
            assert!(
                joined.contains(expected),
                "the variance summary must publish {expected}: {joined}"
            );
        }

        assert!(
            variance_rows(&[0.5]).is_empty(),
            "a single measurement has no variance to publish"
        );
    }

    #[test]
    fn repetition_labels_stay_clean_for_a_single_sweep() {
        assert_eq!(
            all_repetitions_label(1),
            "",
            "a single sweep must not add any repetition noise"
        );
        assert_eq!(turns_evaluated_line(66, 1, 66), "Turns evaluated: 66");
        assert_eq!(coverage_line(60, 66, 1), "Coverage: 90.9% (60/66)");
    }

    #[test]
    fn repetition_labels_mark_accumulated_reports() {
        assert_eq!(all_repetitions_label(3), " (all repetitions)");
        assert_eq!(
            turns_evaluated_line(198, 3, 66),
            "Turns evaluated: 198 (3 repetitions of 66)"
        );
        assert_eq!(
            coverage_line(180, 198, 3),
            "Coverage (pooled over 3 repetitions): 90.9% (180/198)"
        );
    }

    #[test]
    fn parse_args_reads_repeat() {
        let parsed = parse_args(&["--repeat".to_string(), "3".to_string()]);
        assert!(parsed.is_ok(), "--repeat must be accepted");
        assert_eq!(parsed.unwrap().repeat, 3);

        let default = parse_args(&[]).expect("no arguments is valid");
        assert_eq!(
            default.repeat, 1,
            "the default repeat is a single measurement"
        );

        assert!(
            parse_args(&["--repeat".to_string()]).is_err(),
            "--repeat without a value must be rejected"
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

    // ─── Diagnóstico del fallo (route-eval-campaign-support 1.1–1.3) ────────

    /// Un diagnóstico mínimo, para los tests de bandas: probabilidad y umbral
    /// efectivo explícitos, porque la banda mide la **distancia** entre ambos.
    fn diag(prob: Option<f32>, threshold: f32) -> MissingTool {
        MissingTool {
            tool: "x".to_string(),
            skill: Skill::Agenda,
            probability: prob,
            threshold,
            source: SelectionSource::Router,
        }
    }

    #[test]
    fn missing_tool_diagnostic_reports_skill_probability_and_threshold() {
        // A turn that used `tasks` while the router exposed only the core set:
        // `tasks` is missing and `pendientes` is the skill that would cover it.
        let selection = Selection {
            skills: Vec::new(),
            probabilities: vec![(Skill::Pendientes, 0.08), (Skill::Agenda, 0.30)],
            source: SelectionSource::Router,
            usage: None,
        };
        let config = SkillRouterConfig {
            threshold: 0.10,
            ..Default::default()
        };
        let enabled = vec![
            "get_current_time".to_string(),
            "get_current_location".to_string(),
            "tasks".to_string(),
        ];

        let diags = missing_diagnostics(&["tasks".to_string()], &selection, &enabled, &config);

        assert_eq!(
            diags.len(),
            1,
            "one missing tool, one diagnostic: {diags:?}"
        );
        assert_eq!(diags[0].tool, "tasks");
        assert_eq!(
            diags[0].skill,
            Skill::Pendientes,
            "the diagnostic must name the skill that would cover the tool"
        );
        assert_eq!(
            diags[0].probability,
            Some(0.08),
            "the diagnostic must carry the probability the classifier gave the skill"
        );
        assert_eq!(
            diags[0].threshold, 0.10,
            "the diagnostic must carry the effective threshold it was compared against"
        );
    }

    #[test]
    fn near_miss_proximity_is_aggregated_in_bands() {
        // Distances, not raw probabilities: the two ruled-out diagnostics below
        // share similar raw probabilities (0.06 vs 0.09) but sit at very
        // different distances because their thresholds differ (0.10 vs 0.20).
        let diags = vec![
            diag(Some(0.06), 0.10), // distance 0.04
            diag(Some(0.09), 0.20), // distance 0.11
            diag(None, 0.10),       // no probability: never counted as a near miss
        ];

        let bands = near_miss_bands(&diags, &[0.05, 0.15]);

        assert_eq!(
            bands,
            vec![(0.05, 1), (0.15, 2)],
            "each band counts the diagnostics within that distance of their threshold"
        );
    }

    #[test]
    fn uncovered_turn_reports_the_selection_source() {
        let config = SkillRouterConfig {
            threshold: 0.10,
            ..Default::default()
        };
        let enabled = vec![
            "get_current_time".to_string(),
            "get_current_location".to_string(),
            "weather".to_string(),
        ];

        // A router decision that did not select `entorno`: `weather` is used but
        // not exposed, and the source is the router.
        let router_selection = Selection {
            skills: Vec::new(),
            probabilities: vec![(Skill::Entorno, 0.05)],
            source: SelectionSource::Router,
            usage: None,
        };
        let router_diags = missing_diagnostics(
            &["weather".to_string()],
            &router_selection,
            &enabled,
            &config,
        );
        assert_eq!(router_diags.len(), 1);
        assert_eq!(router_diags[0].source, SelectionSource::Router);
        assert_eq!(router_diags[0].skill, Skill::Entorno);

        // A fall-open selection (`Error`) exposes every enabled tool, so only a
        // used tool that is NOT enabled can be missing — and it must report the
        // `Error` source.
        let error_selection = Selection {
            skills: Vec::new(),
            probabilities: Vec::new(),
            source: SelectionSource::Error,
            usage: None,
        };
        let error_diags =
            missing_diagnostics(&["tasks".to_string()], &error_selection, &enabled, &config);
        assert_eq!(error_diags.len(), 1);
        assert_eq!(error_diags[0].source, SelectionSource::Error);
        assert_eq!(
            error_diags[0].probability, None,
            "an unprompted skill has no probability"
        );
    }

    // ─── Overrides desde fichero (route-eval-campaign-support 2.1–2.4) ──────

    const OVERRIDES_JSON: &str = r#"{
        "thresholds": {"global": 0.30, "widgets": 0.25},
        "criteria": {
            "agenda": {
                "instructions": "¿agenda sobrescrita?",
                "criteria_true": "sí",
                "criteria_false": "no"
            }
        }
    }"#;

    #[test]
    fn overrides_file_changes_thresholds_and_criteria_without_mutating_settings() {
        let overrides = parse_overrides(OVERRIDES_JSON).expect("valid overrides file");

        assert_eq!(overrides.thresholds.get("global"), Some(&0.30));
        assert_eq!(overrides.thresholds.get("widgets"), Some(&0.25));
        assert_eq!(
            overrides
                .criteria
                .get("agenda")
                .map(|c| c.instructions.as_str()),
            Some("¿agenda sobrescrita?")
        );

        // Merging the file over the settings returns new maps; the settings map
        // handed in must not be mutated.
        let mut settings_thresholds: HashMap<String, f32> = HashMap::new();
        settings_thresholds.insert("global".to_string(), 0.10);
        settings_thresholds.insert("widgets".to_string(), 0.20);
        let thresholds_before = settings_thresholds.clone();

        let merged = effective_thresholds(&settings_thresholds, &overrides);
        assert_eq!(
            merged.get("global"),
            None,
            "the global threshold is resolved separately, never merged into threshold_overrides"
        );
        assert_eq!(merged.get("widgets"), Some(&0.25));
        assert_eq!(
            settings_thresholds, thresholds_before,
            "the settings map must not be mutated"
        );

        let settings_criteria: HashMap<String, SkillCriteria> = HashMap::new();
        let criteria_before = settings_criteria.clone();
        let merged_criteria = effective_criteria(&settings_criteria, &overrides);
        assert!(merged_criteria.contains_key("agenda"));
        assert_eq!(
            settings_criteria, criteria_before,
            "the settings criteria must not be mutated"
        );
    }

    #[test]
    fn override_precedence_is_cli_over_file_over_settings() {
        assert_eq!(
            resolve_threshold(Some(0.90), Some(0.50), 0.10),
            0.90,
            "the CLI wins over the file and settings"
        );
        assert_eq!(
            resolve_threshold(None, Some(0.50), 0.10),
            0.50,
            "without a CLI value the file wins over settings"
        );
        assert_eq!(
            resolve_threshold(None, None, 0.10),
            0.10,
            "without CLI nor file, settings is used"
        );
    }

    #[test]
    fn missing_overrides_file_fails_loudly() {
        let err = load_overrides("/nonexistent/valet-route-eval-overrides.json")
            .expect_err("a declared but missing file must fail");
        assert!(
            !err.is_empty(),
            "the error must be explicit and non-empty: {err:?}"
        );
    }

    #[test]
    fn report_declares_active_overrides_and_their_origin() {
        let overrides = parse_overrides(OVERRIDES_JSON).expect("valid overrides file");
        let rows = overrides_report("file", &overrides.thresholds, &overrides.criteria, None);
        let joined = rows.join("\n");

        assert!(!rows.is_empty(), "the active overrides must be reported");
        for needle in ["global", "widgets", "agenda", "file"] {
            assert!(
                joined.contains(needle),
                "the report must declare {needle}: {joined}"
            );
        }
    }

    #[test]
    fn report_declares_cli_overrides_with_the_cli_origin() {
        let thresholds: HashMap<String, f32> =
            [("global".to_string(), 0.90f32)].into_iter().collect();
        let criteria: HashMap<String, SkillCriteria> = HashMap::new();

        let rows = overrides_report("cli", &thresholds, &criteria, Some("typesafe/jev-2.0"));
        let joined = rows.join("\n");

        assert!(
            joined.contains("threshold[global] = 0.90 (from cli)"),
            "the CLI threshold and its origin must be declared: {joined}"
        );
        assert!(
            joined.contains("model = typesafe/jev-2.0 (from cli)"),
            "the CLI model and its origin must be declared: {joined}"
        );
    }

    #[test]
    fn parse_overrides_rejects_out_of_range_thresholds() {
        // A threshold outside `[0, 1]` — or one that overflows f32 to infinity —
        // must be a hard error that names the culprit key: measuring with it
        // would silently mislabel the router (never/always selected).
        let high = parse_overrides(r#"{"thresholds": {"widgets": 100}}"#)
            .expect_err("an out-of-range threshold must be rejected");
        assert!(
            high.contains("widgets"),
            "the error must name the culprit key: {high}"
        );
        assert!(
            high.contains("100"),
            "the error must show the value: {high}"
        );

        let negative = parse_overrides(r#"{"thresholds": {"global": -1}}"#)
            .expect_err("a negative threshold must be rejected");
        assert!(
            negative.contains("global"),
            "the error must name the culprit key: {negative}"
        );

        let infinite = parse_overrides(r#"{"thresholds": {"web": 1e40}}"#)
            .expect_err("a value that overflows to infinity must be rejected");
        assert!(
            infinite.contains("web"),
            "the error must name the culprit key: {infinite}"
        );
    }

    #[test]
    fn parse_overrides_rejects_malformed_json_and_wrong_types() {
        assert!(
            parse_overrides("{ not json").is_err(),
            "malformed JSON must be rejected"
        );
        assert!(
            parse_overrides("[1, 2, 3]").is_err(),
            "a non-object JSON must be rejected"
        );
        assert!(
            parse_overrides(r#"{"thresholds": 5}"#).is_err(),
            "non-object thresholds must be rejected"
        );
        assert!(
            parse_overrides(r#"{"criteria": 5}"#).is_err(),
            "non-object criteria must be rejected"
        );
    }

    #[test]
    fn parse_args_reads_overrides() {
        let parsed = parse_args(&["--overrides".to_string(), "overrides.json".to_string()])
            .expect("--overrides must be accepted");
        assert_eq!(parsed.overrides.as_deref(), Some("overrides.json"));

        assert!(
            parse_args(&["--overrides".to_string()]).is_err(),
            "--overrides without a value must be rejected"
        );
    }

    #[test]
    fn effective_thresholds_drops_the_global_threshold() {
        // The file global lives in `router_config.threshold` (via
        // `resolve_threshold`), so the per-skill map must not carry it.
        let mut settings: HashMap<String, f32> = HashMap::new();
        settings.insert("global".to_string(), 0.10);
        settings.insert("widgets".to_string(), 0.20);

        let overrides = Overrides {
            thresholds: [
                ("global".to_string(), 0.30f32),
                ("widgets".to_string(), 0.25),
            ]
            .into_iter()
            .collect(),
            criteria: HashMap::new(),
        };

        let merged = effective_thresholds(&settings, &overrides);
        assert_eq!(
            merged.get("global"),
            None,
            "the file global must not remain in threshold_overrides after the merge"
        );
        assert_eq!(merged.get("widgets"), Some(&0.25));
    }

    #[test]
    fn near_miss_band_includes_the_exact_upper_boundary() {
        // In f32 `0.10 - 0.08 = 0.020000003 > 0.02`, so a strict `<=` would miss
        // the boundary the label promises. The tolerance must keep it counted.
        let diags = vec![diag(Some(0.08), 0.10)];
        assert_eq!(
            near_miss_bands(&diags, &[0.02]),
            vec![(0.02, 1)],
            "a near miss exactly on the band edge must be counted"
        );
    }
}
