//! El `JournalWorker`: la bitácora que la app escribe por su cuenta.
//!
//! El worker juzga **un día por vuelta** —el día cerrado más antiguo que aún no
//! se haya juzgado— con el material de ese día (la entrada anterior, los hechos
//! del timeline y los mensajes) y, cuando el modelo responde que hay algo que
//! contar, escribe una entrada ordinaria en `journal_entries` con
//! [`crate::db::repos::journal::JournalRepo::upsert`].
//!
//! Este módulo fija la superficie de la parte **pura** del worker, que es la que
//! decide *qué* día toca:
//!
//! - [`last_closed_day`] — el último día cerrado según la hora de corte y la
//!   zona del usuario.
//! - [`target_day`] — el día que toca juzgar, comparando la marca de progreso
//!   con el último día cerrado.
//! - [`day_window`] — la ventana UTC `[inicio, fin)` de un día local, que cubre
//!   el día entero aunque dure 23 o 25 horas por un cambio de hora.
//! - [`JournalSettings`] — los ajustes de la bitácora leídos **en cada vuelta**.
//!
//! # Estado (fase GREEN — tareas 6.1–6.4)
//!
//! La parte pura del worker está implementada: el día local con su hora de
//! corte y su zona ([`last_closed_day`]), el día objetivo ([`target_day`]), la
//! ventana UTC del día ([`day_window`]), la lectura de ajustes en cada vuelta
//! ([`JournalSettings::load`]) y el respaldo mínimo del prompt
//! ([`DEFAULT_JOURNAL_PROMPT_FALLBACK`]). Nada de esto toca la base de datos ni
//! el reloj del sistema salvo por lo que se le pasa como parámetro.
//!
//! El worker en sí (el ciclo, el material, la escritura y el fallo) llega en
//! los grupos 4–6 de `tasks.md`, en otro encargo.

use std::str::FromStr;

use chrono::{DateTime, NaiveDate, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use sqlx::SqlitePool;

use crate::db::repos::settings::SettingsRepo;

/// La hora de corte por defecto (`journal_hour`): el día anterior ya está
/// cerrado cuando, en la zona del usuario, se ha pasado esta hora del día
/// siguiente.
pub const DEFAULT_JOURNAL_HOUR: u8 = 4;

/// Cuánto puede desplazarse el inicio de un día local cuando su medianoche no
/// existe (la zona cambia de hora justo entonces): se toma el primer instante
/// válido de ese día, buscándolo como mucho estos minutos hacia adelante.
const MAX_LOCAL_MIDNIGHT_SHIFT_MINUTES: i64 = 180;

/// Respaldo mínimo del prompt de la bitácora, usado solo cuando
/// `settings.journal_prompt` falta o está en blanco.
///
/// El prompt de verdad vive en la base de datos (lo siembra una migración) y
/// pide **el mismo contrato de salida** que este respaldo: el objeto JSON con
/// `write`, el relato y los campos opcionales, y el `{ "write": false }` de la
/// jornada que no tiene nada que contar.
pub const DEFAULT_JOURNAL_PROMPT_FALLBACK: &str = r#"Eres el diario del autor. Recibes el material de una jornada: los hechos ya registrados, los mensajes del día y la entrada anterior del diario.

Escribe el relato del día en primera persona, como lo contaría su propio autor: cómo fue, qué costó y qué ilusionó. Los hechos del día ya están registrados en el timeline: no los enumeres otra vez, descríbelos. Describe en vez de diagnosticar, y no inventes nada que no esté en el material.

Devuelve EXCLUSIVAMENTE un objeto JSON:
- Si la jornada no tiene nada que contar: {"write": false}
- Si hay algo que contar: {"write": true, "content": "el relato del día", "mood": "great|good|neutral|bad|terrible", "achievements": ["…"], "struggles": ["…"], "tags": ["…"]}

"write" es obligatorio y booleano. Con "write" en true, "content" es el relato en primera persona y no puede estar vacío. "mood", "achievements", "struggles" y "tags" son opcionales."#;

/// Los ajustes de la bitácora tal como el worker los necesita en **una** vuelta.
///
/// Se leen al empezar cada vuelta (`load`), nunca al arrancar: cambiar
/// `journal_hour` o `timezone` surte efecto en la vuelta siguiente sin reiniciar
/// el worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalSettings {
    /// `journal_enabled`: la bitácora está apagada **solo** con `'false'`.
    pub enabled: bool,
    /// `journal_hour`: la hora local a partir de la cual el día anterior ya está
    /// cerrado. Un valor inservible cae a `4`.
    pub cut_hour: u8,
    /// `journal_prompt`: el prompt de sistema de la llamada.
    pub prompt: String,
    /// `journal_last_judged_day`: la marca de progreso. `None` significa «nunca
    /// se ha juzgado nada».
    pub last_judged_day: Option<String>,
    /// `timezone`: la zona IANA del usuario, tal como está en `settings`. La
    /// caída a UTC la hacen las funciones puras, que son el único sitio donde
    /// una zona ausente o inválida se resuelve.
    pub timezone: String,
    /// Los avisos que dejó la interpretación de los ajustes: un valor
    /// irreconocible que cayó a su defecto. **Cada aviso nombra la clave del
    /// ajuste que lo provocó** (`journal_hour`, `journal_prompt`, …), para que
    /// el worker los registre y para que se puedan afirmar en los tests.
    ///
    /// SQLx no revienta la vuelta: un ajuste ausente, vacío o ilegible cae a su
    /// defecto con su aviso.
    pub warnings: Vec<String>,
}

impl JournalSettings {
    /// Lee los ajustes de la bitácora de `settings`, **en cada llamada**.
    ///
    /// Un ajuste ausente, vacío o ilegible cae a su defecto y deja su aviso en
    /// [`JournalSettings::warnings`] —que nombra siempre la clave culpable—
    /// para que el worker lo registre. Una lectura que falle se trata como
    /// ausencia: la vuelta se degrada, pero nunca revienta.
    pub async fn load(db: &SqlitePool) -> Self {
        let mut warnings = Vec::new();

        let enabled = match read_setting(db, "journal_enabled").await {
            None => true,
            Some(raw) => match raw.trim().to_ascii_lowercase().as_str() {
                "false" => false,
                // Un valor vacío es «sin valor»: el defecto, sin aviso. Solo un
                // valor presente pero irreconocible avisa.
                "true" | "" => true,
                _ => {
                    warnings.push(format!(
                        "settings.journal_enabled='{raw}' is not a boolean; keeping the journal on"
                    ));
                    true
                }
            },
        };

        let cut_hour = match read_setting(db, "journal_hour").await {
            None => DEFAULT_JOURNAL_HOUR,
            Some(raw) => match raw.trim().parse::<u8>() {
                Ok(hour) if hour <= 23 => hour,
                _ => {
                    warnings.push(format!(
                        "settings.journal_hour='{raw}' is not an hour between 0 and 23; \
                         using {DEFAULT_JOURNAL_HOUR}"
                    ));
                    DEFAULT_JOURNAL_HOUR
                }
            },
        };

        let prompt = match read_setting(db, "journal_prompt").await {
            Some(raw) if !raw.trim().is_empty() => raw,
            Some(raw) => {
                warnings.push(format!(
                    "settings.journal_prompt='{raw}' is blank; using the minimal fallback"
                ));
                DEFAULT_JOURNAL_PROMPT_FALLBACK.to_string()
            }
            None => {
                warnings
                    .push("settings.journal_prompt is missing; using the minimal fallback".into());
                DEFAULT_JOURNAL_PROMPT_FALLBACK.to_string()
            }
        };

        let last_judged_day = match read_setting(db, "journal_last_judged_day").await {
            None => None,
            Some(raw) => match parse_iso_date(&raw) {
                Some(day) => Some(day.format(ISO_DATE).to_string()),
                None => {
                    warnings.push(format!(
                        "settings.journal_last_judged_day='{raw}' is not an ISO date; \
                         treating it as never judged"
                    ));
                    None
                }
            },
        };

        // La zona se guarda tal cual: la caída a UTC la hacen las funciones
        // puras, que es el único sitio donde una zona ausente o inválida se
        // resuelve.
        let timezone = read_setting(db, "timezone").await.unwrap_or_default();

        Self {
            enabled,
            cut_hour,
            prompt,
            last_judged_day,
            timezone,
            warnings,
        }
    }

    /// El día que toca juzgar en esta vuelta, o `None` cuando no hay nada
    /// pendiente: el último día cerrado en la zona del usuario, recortado por la
    /// marca de progreso ([`target_day`]).
    ///
    /// No mira [`JournalSettings::enabled`]: apagar la bitácora es cosa del
    /// worker, que es quien decide si llega a llamar al modelo.
    pub fn target(&self, now_utc: DateTime<Utc>) -> Option<String> {
        let closed_day = last_closed_day(now_utc, &self.timezone, self.cut_hour);
        target_day(self.last_judged_day.as_deref(), &closed_day)
    }
}

/// Lee una clave de `settings`. Un fallo de lectura se trata como ausencia.
async fn read_setting(db: &SqlitePool, key: &str) -> Option<String> {
    SettingsRepo::get(db, key).await.ok().flatten()
}

/// El formato de las fechas de la bitácora: el día se direcciona por fecha ISO.
const ISO_DATE: &str = "%Y-%m-%d";

/// Una fecha `YYYY-MM-DD`, o `None` si no encaja en el formato. Sin fecha
/// inventada.
fn parse_iso_date(date: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(date.trim(), ISO_DATE).ok()
}

/// La zona IANA pedida, o UTC si falta o no es válida.
fn resolve_timezone(timezone: &str) -> Tz {
    Tz::from_str(timezone.trim()).unwrap_or(chrono_tz::UTC)
}

/// El primer instante de un día local, en UTC.
///
/// Una medianoche local ambigua (la zona atrasa el reloj justo entonces) se
/// resuelve con la primera de sus dos ocurrencias, y una inexistente (la zona
/// adelanta el reloj justo entonces) con el primer instante válido del día.
fn local_midnight(tz: &Tz, day: NaiveDate) -> Option<DateTime<Utc>> {
    let midnight = day.and_hms_opt(0, 0, 0)?;
    for minutes in 0..=MAX_LOCAL_MIDNIGHT_SHIFT_MINUTES {
        let naive = midnight.checked_add_signed(chrono::Duration::minutes(minutes))?;
        if let Some(local) = tz.from_local_datetime(&naive).earliest() {
            return Some(local.with_timezone(&Utc));
        }
    }
    None
}

/// El día local cerrado más reciente: `D` está cerrado cuando en la zona del
/// usuario ya se ha pasado la hora de corte del día siguiente. Una zona ausente
/// o inválida cae a UTC.
///
/// Con la hora de corte por defecto (4), a las 04:05 del 19 el día cerrado es el
/// 18, y a las 03:59 sigue siendo el 17.
pub fn last_closed_day(now_utc: DateTime<Utc>, timezone: &str, cut_hour: u8) -> String {
    let tz = resolve_timezone(timezone);
    let local = now_utc.with_timezone(&tz);
    let today = local.date_naive();

    let closed = if local.hour() >= u32::from(cut_hour) {
        // La hora de corte de hoy ya ha pasado: el día anterior está cerrado.
        today.pred_opt()
    } else {
        // Todavía no: el último día cerrado es el de antes de ayer.
        today.pred_opt().and_then(|yesterday| yesterday.pred_opt())
    };

    closed.unwrap_or(today).format(ISO_DATE).to_string()
}

/// El día que toca juzgar: el más antiguo posterior a `last_judged`, sin
/// pasarse de `closed_day`. `None` cuando no hay nada que juzgar. Sin marca de
/// progreso, el objetivo es `closed_day` (nunca el principio de los tiempos).
///
/// Una marca ilegible también se trata como «nunca juzgado»: el objetivo vuelve
/// a ser el último día cerrado, no el principio de los tiempos.
pub fn target_day(last_judged: Option<&str>, closed_day: &str) -> Option<String> {
    let closed = parse_iso_date(closed_day)?;

    let next = match last_judged.and_then(parse_iso_date) {
        Some(judged) => judged.succ_opt()?,
        None => closed,
    };

    if next > closed {
        None
    } else {
        Some(next.format(ISO_DATE).to_string())
    }
}

/// La ventana UTC `[inicio, fin)` de un día local `YYYY-MM-DD`, o `None` si la
/// fecha no encaja en el formato. Una zona ausente o inválida cae a UTC.
///
/// La ventana cubre el día local completo **aunque dure 23 o 25 horas** por un
/// cambio de hora: el inicio es la medianoche local de `date` y el fin la de
/// `date + 1`.
pub fn day_window(date: &str, timezone: &str) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let day = parse_iso_date(date)?;
    let next_day = day.succ_opt()?;
    let tz = resolve_timezone(timezone);

    let start = local_midnight(&tz, day)?;
    let end = local_midnight(&tz, next_day)?;

    Some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::db::repos::journal::JournalRepo;
    use crate::db::repos::profiles::ProfilesRepo;
    use crate::db::repos::settings::SettingsRepo;
    use crate::db::schema::run_migrations;
    use crate::llm::provider::{
        ChatMessage, ChatRequest, ChatResponse, FinishReason, LLMError, LLMProvider, TokenUsage,
    };
    use crate::models::journal::JournalEntry;
    use crate::models::stats::CallKind;
    use crate::models::Mood;
    use crate::workers::pool::WorkerPool;
    use async_trait::async_trait;
    use chrono::TimeZone;
    use sqlx::Row;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    /// Una base en memoria con las migraciones aplicadas.
    ///
    /// El `CHECK` de `llm_requests.kind` todavía no admite `'journal'`: la
    /// migración que lo ensancha es la tarea 8.1 de `tasks.md`, fuera de este
    /// contrato. Las estadísticas del worker (grupo 5) tienen que poder guardar
    /// filas `kind = 'journal'` **hoy**, así que el arnés de pruebas reconstruye
    /// la tabla con el `CHECK` ensanchado —el mismo procedimiento documentado
    /// que la migración real ejecutará—. Es un shim de test; la tarea 8.1 lo
    /// hace redundante.
    async fn test_db() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await
            .expect("Failed to create in-memory database");
        run_migrations(&pool)
            .await
            .expect("Failed to run migrations");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS llm_requests_test_shim (\
             id TEXT PRIMARY KEY, \
             model TEXT NOT NULL, \
             provider TEXT, \
             profile_id TEXT REFERENCES profiles(id), \
             prompt_tokens INTEGER NOT NULL DEFAULT 0, \
             completion_tokens INTEGER NOT NULL DEFAULT 0, \
             total_tokens INTEGER NOT NULL DEFAULT 0, \
             cached_tokens INTEGER NOT NULL DEFAULT 0, \
             reasoning_tokens INTEGER NOT NULL DEFAULT 0, \
             cost REAL NOT NULL DEFAULT 0.0, \
             is_byok INTEGER NOT NULL DEFAULT 0, \
             duration_ms INTEGER, \
             cache_hit INTEGER NOT NULL DEFAULT 0, \
             status TEXT NOT NULL DEFAULT 'success' CHECK(status IN ('success', 'error', 'timeout')), \
             error_message TEXT, \
             tool_calls TEXT, \
             created_at TEXT NOT NULL DEFAULT (datetime('now')), \
             kind TEXT NOT NULL DEFAULT 'chat' \
               CHECK (kind IN ('chat', 'router', 'archivist', 'consolidator', 'collapse', 'timeline', 'journal')))",
        )
        .execute(&pool)
        .await
        .expect("Failed to create the widened llm_requests shim");
        sqlx::query("INSERT INTO llm_requests_test_shim SELECT * FROM llm_requests")
            .execute(&pool)
            .await
            .expect("Failed to copy llm_requests rows into the shim");
        sqlx::query("DROP TABLE llm_requests")
            .execute(&pool)
            .await
            .expect("Failed to drop the old llm_requests");
        sqlx::query("ALTER TABLE llm_requests_test_shim RENAME TO llm_requests")
            .execute(&pool)
            .await
            .expect("Failed to rename the shim over llm_requests");

        pool
    }

    /// Escribe una clave de `settings`.
    async fn put(db: &SqlitePool, key: &str, value: &str) {
        SettingsRepo::set(db, key, value)
            .await
            .expect("Failed to write a setting");
    }

    /// El instante UTC que corresponde a una hora local de `Europe/Madrid`.
    fn madrid_at(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> DateTime<Utc> {
        chrono_tz::Europe::Madrid
            .with_ymd_and_hms(year, month, day, hour, minute, 0)
            .single()
            .expect("a local instant that exists exactly once")
            .with_timezone(&Utc)
    }

    /// Un instante UTC a partir de su forma RFC 3339.
    fn utc_at(rfc3339: &str) -> DateTime<Utc> {
        rfc3339
            .parse::<DateTime<Utc>>()
            .expect("a valid RFC 3339 instant")
    }

    /// `true` si `instant` cae en la ventana `[inicio, fin)`: el inicio entra y
    /// el fin no, porque el fin ya es el inicio del día siguiente.
    fn belongs_to(window: (DateTime<Utc>, DateTime<Utc>), instant: DateTime<Utc>) -> bool {
        let (start, end) = window;
        start <= instant && instant < end
    }

    // ════════════════════════════════════════════════════════════════════════
    // 1.1 — el día, la hora de corte y la zona
    // ════════════════════════════════════════════════════════════════════════

    /// Scenario: A las 04:05 del 19 de octubre se juzga el 18.
    #[test]
    fn test_last_closed_day_after_the_cut_hour_is_the_previous_day() {
        let now = madrid_at(2026, 10, 19, 4, 5);
        assert_eq!(
            last_closed_day(now, "Europe/Madrid", 4),
            "2026-10-18",
            "a las 04:05 locales el día anterior ya está cerrado"
        );
    }

    /// Scenario: Antes de la hora de corte el día anterior todavía no está cerrado.
    #[test]
    fn test_last_closed_day_before_the_cut_hour_leaves_the_previous_day_open() {
        let now = madrid_at(2026, 10, 19, 3, 59);
        assert_eq!(
            last_closed_day(now, "Europe/Madrid", 4),
            "2026-10-17",
            "a las 03:59 locales el 18 todavía no está cerrado"
        );
    }

    /// Scenario: Con la hora de corte a 0 el día se cierra al empezar el siguiente.
    #[test]
    fn test_last_closed_day_with_cut_hour_zero_closes_at_midnight() {
        let now = madrid_at(2026, 10, 19, 0, 0);
        assert_eq!(
            last_closed_day(now, "Europe/Madrid", 0),
            "2026-10-18",
            "con la hora de corte a 0 el día se cierra al empezar el siguiente"
        );
    }

    // ════════════════════════════════════════════════════════════════════════
    // 1.2 — la zona ausente o inválida
    // ════════════════════════════════════════════════════════════════════════

    /// Scenario: Una zona ausente o inválida cae a UTC.
    #[test]
    fn test_last_closed_day_with_an_invalid_timezone_falls_back_to_utc() {
        let now = utc_at("2026-10-19T05:00:00Z");
        assert_eq!(
            last_closed_day(now, "Marte/Olympus", 4),
            last_closed_day(now, "UTC", 4),
            "una zona inválida no inventa una fecha: cae a UTC"
        );
        assert_eq!(last_closed_day(now, "Marte/Olympus", 4), "2026-10-18");
    }

    /// Scenario: Una zona ausente o inválida cae a UTC.
    #[test]
    fn test_last_closed_day_with_a_missing_timezone_falls_back_to_utc() {
        let now = utc_at("2026-10-19T01:00:00Z");
        assert_eq!(
            last_closed_day(now, "", 4),
            last_closed_day(now, "UTC", 4),
            "una zona ausente cae a UTC"
        );
        assert_eq!(
            last_closed_day(now, "", 4),
            "2026-10-17",
            "01:00 UTC con la hora de corte a 4: el 18 aún no está cerrado"
        );
    }

    // ════════════════════════════════════════════════════════════════════════
    // 1.3 — la ventana del día, cambios de hora incluidos
    // ════════════════════════════════════════════════════════════════════════

    /// Scenario: La ventana de un día normal dura 24 horas.
    #[test]
    fn test_day_window_of_a_plain_day_lasts_twenty_four_hours() {
        let (start, end) =
            day_window("2026-10-18", "Europe/Madrid").expect("a well-formed date has a window");
        assert_eq!(start, utc_at("2026-10-17T22:00:00Z"));
        assert_eq!(end, utc_at("2026-10-18T22:00:00Z"));
        assert_eq!(end - start, chrono::Duration::hours(24));
    }

    /// Scenario: Un cambio de hora no descuadra la ventana (25 horas).
    #[test]
    fn test_day_window_covers_the_twenty_five_hour_day() {
        let (start, end) =
            day_window("2026-10-25", "Europe/Madrid").expect("a well-formed date has a window");
        assert_eq!(
            start,
            utc_at("2026-10-24T22:00:00Z"),
            "el inicio es la medianoche local del 25"
        );
        assert_eq!(
            end,
            utc_at("2026-10-25T23:00:00Z"),
            "el fin es la medianoche local del 26"
        );
        assert_eq!(end - start, chrono::Duration::hours(25));
    }

    /// Scenario: Un cambio de hora no descuadra la ventana (23 horas).
    #[test]
    fn test_day_window_covers_the_twenty_three_hour_day() {
        let (start, end) =
            day_window("2026-03-29", "Europe/Madrid").expect("a well-formed date has a window");
        assert_eq!(start, utc_at("2026-03-28T23:00:00Z"));
        assert_eq!(end, utc_at("2026-03-29T22:00:00Z"));
        assert_eq!(end - start, chrono::Duration::hours(23));
    }

    /// Scenario: Una fecha mal formada no tiene ventana.
    #[test]
    fn test_day_window_of_a_malformed_date_is_none() {
        for raw in ["18/10/2026", "ayer", "", "2026-10-32", "2026-13-01"] {
            assert!(
                day_window(raw, "Europe/Madrid").is_none(),
                "«{raw}» no es una fecha ISO: no hay ventana ni fecha inventada"
            );
        }
    }

    /// Scenario: Una zona ausente o inválida cae a UTC.
    #[test]
    fn test_day_window_with_an_invalid_timezone_falls_back_to_utc() {
        let madrid_window = day_window("2026-10-18", "Europe/Madrid").expect("a valid window");
        let utc_window = day_window("2026-10-18", "UTC").expect("a valid window");
        assert_ne!(madrid_window, utc_window);

        assert_eq!(
            day_window("2026-10-18", "Marte/Olympus"),
            Some(utc_window),
            "una zona inválida cae a UTC"
        );
        assert_eq!(
            day_window("2026-10-18", ""),
            Some((
                utc_at("2026-10-18T00:00:00Z"),
                utc_at("2026-10-19T00:00:00Z")
            )),
            "una zona ausente cae a UTC"
        );
    }

    // ════════════════════════════════════════════════════════════════════════
    // 1.4 — la pertenencia al día
    // ════════════════════════════════════════════════════════════════════════

    /// Scenario: La pertenencia al día se decide en UTC.
    #[test]
    fn test_day_window_membership_is_decided_in_utc() {
        let instant = utc_at("2026-10-18T22:30:00Z");

        let day_19 = day_window("2026-10-19", "Europe/Madrid").expect("a valid window");
        assert!(
            belongs_to(day_19, instant),
            "las 22:30Z son las 00:30 del 19 en Madrid"
        );

        let day_18 = day_window("2026-10-18", "Europe/Madrid").expect("a valid window");
        assert!(
            !belongs_to(day_18, instant),
            "el mismo instante no pertenece al 18"
        );
    }

    /// Scenario: La pertenencia al día se decide en UTC (el inicio entra).
    #[test]
    fn test_day_window_starts_at_its_first_instant_and_ends_where_the_next_one_starts() {
        let (start, end) = day_window("2026-10-18", "Europe/Madrid").expect("a valid window");
        assert!(
            belongs_to((start, end), start),
            "un instante justo al inicio de la ventana cae en su día"
        );

        let (next_start, next_end) =
            day_window("2026-10-19", "Europe/Madrid").expect("a valid window");
        assert_eq!(
            end, next_start,
            "la ventana termina donde empieza la del día siguiente"
        );
        assert!(
            !belongs_to((start, end), next_end),
            "un instante del día siguiente no pertenece a este"
        );
    }

    // ════════════════════════════════════════════════════════════════════════
    // 1.5 — el día objetivo
    // ════════════════════════════════════════════════════════════════════════

    /// Scenario: El objetivo es el día siguiente al último juzgado.
    #[test]
    fn test_target_day_is_the_day_right_after_the_last_judged_one() {
        assert_eq!(
            target_day(Some("2026-10-16"), "2026-10-18").as_deref(),
            Some("2026-10-17")
        );
    }

    /// Scenario: Sin marca de progreso, el primer objetivo es el último día cerrado.
    #[test]
    fn test_target_day_without_a_progress_mark_is_the_closed_day() {
        assert_eq!(
            target_day(None, "2026-10-18").as_deref(),
            Some("2026-10-18"),
            "sin marca no se recorre la historia hacia atrás"
        );
    }

    /// Scenario: Un día juzgado no deja objetivo.
    #[test]
    fn test_target_day_with_the_closed_day_already_judged_is_none() {
        assert_eq!(target_day(Some("2026-10-18"), "2026-10-18"), None);
        assert_eq!(
            target_day(Some("2026-10-20"), "2026-10-18"),
            None,
            "una marca por delante del último día cerrado tampoco deja objetivo"
        );
    }

    // ════════════════════════════════════════════════════════════════════════
    // 2.1 — journal_enabled
    // ════════════════════════════════════════════════════════════════════════

    /// Scenario: Sin el ajuste (o vacío) la bitácora está encendida.
    #[tokio::test]
    async fn test_journal_settings_enabled_by_default() {
        let db = test_db().await;

        let settings = JournalSettings::load(&db).await;
        assert!(
            settings.enabled,
            "sin journal_enabled la bitácora está encendida"
        );
        assert!(
            !settings
                .warnings
                .iter()
                .any(|w| w.contains("journal_enabled")),
            "la ausencia del ajuste es su defecto, no un valor irreconocible: {:?}",
            settings.warnings
        );

        put(&db, "journal_enabled", "").await;
        let settings = JournalSettings::load(&db).await;
        assert!(
            settings.enabled,
            "un journal_enabled vacío deja la bitácora encendida"
        );
    }

    /// Scenario: Solo el valor `false` apaga la bitácora.
    #[tokio::test]
    async fn test_journal_settings_only_the_value_false_turns_the_journal_off() {
        let db = test_db().await;

        for raw in ["false", "FALSE", "false ", " false", "False"] {
            put(&db, "journal_enabled", raw).await;
            let settings = JournalSettings::load(&db).await;
            assert!(
                !settings.enabled,
                "journal_enabled = {raw:?} apaga la bitácora"
            );
            assert!(
                !settings
                    .warnings
                    .iter()
                    .any(|w| w.contains("journal_enabled")),
                "{raw:?} es reconocible: no deja aviso"
            );
        }
    }

    /// Scenario: Un valor irreconocible deja la bitácora encendida.
    #[tokio::test]
    async fn test_journal_settings_unreadable_enabled_value_keeps_the_journal_on_with_a_warning() {
        let db = test_db().await;
        put(&db, "journal_enabled", "quizá").await;

        let settings = JournalSettings::load(&db).await;
        assert!(
            settings.enabled,
            "un valor irreconocible deja la bitácora encendida"
        );
        assert!(
            settings
                .warnings
                .iter()
                .any(|w| w.contains("journal_enabled")),
            "un valor irreconocible deja un aviso: {:?}",
            settings.warnings
        );
    }

    // ════════════════════════════════════════════════════════════════════════
    // 2.2 — journal_hour
    // ════════════════════════════════════════════════════════════════════════

    /// Scenario: Una hora inservible cae a 4.
    #[tokio::test]
    async fn test_journal_settings_hour_falls_back_to_four_with_a_warning() {
        let db = test_db().await;

        for raw in ["25", "mediodía", "", "-1", "04:30"] {
            put(&db, "journal_hour", raw).await;
            let settings = JournalSettings::load(&db).await;
            assert_eq!(settings.cut_hour, 4, "journal_hour = {raw:?} cae a 4");
            assert!(
                settings.warnings.iter().any(|w| w.contains("journal_hour")),
                "journal_hour = {raw:?} deja un aviso: {:?}",
                settings.warnings
            );
        }
    }

    /// Scenario: Un valor válido se respeta, y la ausencia es su defecto.
    #[tokio::test]
    async fn test_journal_settings_valid_hour_is_respected() {
        let db = test_db().await;

        let settings = JournalSettings::load(&db).await;
        assert_eq!(
            settings.cut_hour, 4,
            "sin journal_hour la hora de corte es 4"
        );
        assert!(
            !settings.warnings.iter().any(|w| w.contains("journal_hour")),
            "la ausencia del ajuste es su defecto, no un valor inservible"
        );

        for (raw, expected) in [("0", 0_u8), ("7", 7), ("23", 23)] {
            put(&db, "journal_hour", raw).await;
            let settings = JournalSettings::load(&db).await;
            assert_eq!(settings.cut_hour, expected, "journal_hour = {raw:?}");
            assert!(
                !settings.warnings.iter().any(|w| w.contains("journal_hour")),
                "{raw:?} es reconocible: no deja aviso"
            );
        }
    }

    // ════════════════════════════════════════════════════════════════════════
    // 2.3 — la marca de progreso ilegible
    // ════════════════════════════════════════════════════════════════════════

    /// Scenario: Una marca de progreso ilegible se trata como «nunca juzgado».
    #[tokio::test]
    async fn test_journal_settings_unreadable_progress_mark_is_never_judged_with_a_warning() {
        let db = test_db().await;
        put(&db, "timezone", "Europe/Madrid").await;
        put(&db, "journal_last_judged_day", "ayer").await;

        let settings = JournalSettings::load(&db).await;
        assert_eq!(
            settings.last_judged_day, None,
            "una marca ilegible es «nunca juzgado»"
        );
        assert!(
            settings
                .warnings
                .iter()
                .any(|w| w.contains("journal_last_judged_day")),
            "una marca ilegible deja un aviso: {:?}",
            settings.warnings
        );

        let now = madrid_at(2026, 10, 19, 4, 5);
        assert_eq!(
            settings.target(now).as_deref(),
            Some("2026-10-18"),
            "el objetivo vuelve a ser el último día cerrado, no el principio de los tiempos"
        );
    }

    // ════════════════════════════════════════════════════════════════════════
    // 2.4 — journal_prompt
    // ════════════════════════════════════════════════════════════════════════

    /// Scenario: Un prompt ausente usa el respaldo mínimo.
    #[tokio::test]
    async fn test_journal_prompt_absent_uses_the_code_fallback_with_a_warning() {
        let db = test_db().await;

        let settings = JournalSettings::load(&db).await;
        assert_eq!(settings.prompt, DEFAULT_JOURNAL_PROMPT_FALLBACK);
        assert!(
            !settings.prompt.trim().is_empty(),
            "el respaldo mínimo del código no puede quedar vacío"
        );
        assert!(
            settings
                .warnings
                .iter()
                .any(|w| w.contains("journal_prompt")),
            "un prompt ausente deja un aviso: {:?}",
            settings.warnings
        );
    }

    /// Scenario: Un prompt en blanco usa el respaldo mínimo.
    #[tokio::test]
    async fn test_journal_prompt_blank_uses_the_code_fallback_with_a_warning() {
        let db = test_db().await;
        put(&db, "journal_prompt", "  \n\t ").await;

        let settings = JournalSettings::load(&db).await;
        assert_eq!(settings.prompt, DEFAULT_JOURNAL_PROMPT_FALLBACK);
        assert!(
            settings
                .warnings
                .iter()
                .any(|w| w.contains("journal_prompt")),
            "un prompt en blanco deja un aviso: {:?}",
            settings.warnings
        );
    }

    /// Un prompt propio se respeta tal cual.
    #[tokio::test]
    async fn test_journal_prompt_from_settings_is_respected() {
        let db = test_db().await;
        put(&db, "journal_prompt", "Escribe el diario del día.").await;

        let settings = JournalSettings::load(&db).await;
        assert_eq!(settings.prompt, "Escribe el diario del día.");
        assert!(
            !settings
                .warnings
                .iter()
                .any(|w| w.contains("journal_prompt")),
            "un prompt propio no deja aviso"
        );
    }

    // ════════════════════════════════════════════════════════════════════════
    // 2.6 — la lectura en caliente
    // ════════════════════════════════════════════════════════════════════════

    /// Scenario: Los ajustes se releen en cada vuelta.
    #[tokio::test]
    async fn test_journal_settings_are_read_on_every_pass() {
        let db = test_db().await;
        put(&db, "timezone", "Europe/Madrid").await;
        put(&db, "journal_hour", "4").await;

        // Las 04:00 locales del 19: con la hora de corte a 4 el 18 ya está cerrado.
        let now = madrid_at(2026, 10, 19, 4, 0);
        let first_pass = JournalSettings::load(&db).await;
        assert_eq!(
            first_pass.target(now).as_deref(),
            Some("2026-10-18"),
            "con journal_hour = 4 el objetivo es el 18"
        );

        // La vuelta siguiente lee el ajuste nuevo, sin reiniciar el worker.
        put(&db, "journal_hour", "23").await;
        let second_pass = JournalSettings::load(&db).await;
        assert_eq!(
            second_pass.cut_hour, 23,
            "la vuelta siguiente lee el journal_hour nuevo"
        );
        assert_eq!(
            second_pass.target(now).as_deref(),
            Some("2026-10-17"),
            "con journal_hour = 23 el 18 aún no está cerrado"
        );
    }

    // ════════════════════════════════════════════════════════════════════════
    // 3.4 — el respaldo mínimo del código
    // ════════════════════════════════════════════════════════════════════════

    /// Scenario: El respaldo del código es coherente con el contrato.
    ///
    /// El texto sembrado de `journal_prompt` vive en la migración (grupo 8 de
    /// `tasks.md`, tareas 3.1–3.3): aquí solo se fija que el respaldo del código
    /// pide **el mismo** contrato de salida, no una forma distinta.
    #[test]
    fn test_journal_prompt_fallback_declares_the_output_contract() {
        let prompt = DEFAULT_JOURNAL_PROMPT_FALLBACK;

        assert!(
            !prompt.trim().is_empty(),
            "el respaldo mínimo del código no puede quedar vacío"
        );
        for needle in ["\"write\"", "\"content\"", "\"mood\"", "\"write\": false"] {
            assert!(
                prompt.contains(needle),
                "el respaldo mínimo debe pedir el contrato de salida; le falta {needle}"
            );
        }
    }

    // ════════════════════════════════════════════════════════════════════════
    // Grupos 4 y 5 — el worker: material, escritura, idempotencia, apagado,
    // el fallo y las estadísticas (RED)
    // ════════════════════════════════════════════════════════════════════════

    /// El modelo con el que el worker llama en los tests.
    const TEST_JOURNAL_MODEL: &str = "test/journal-model";

    /// Un contrato de salida válido: relato con ánimo.
    fn valid_entry_json() -> &'static str {
        r#"{"write": true, "content": "El relato del día.", "mood": "good"}"#
    }

    /// El turno `user` de una llamada: ahí viaja el bloque de material.
    fn user_turn(request: &ChatRequest) -> String {
        request
            .messages
            .iter()
            .find(|m| m.role == "user")
            .map(|m| m.content.clone())
            .unwrap_or_default()
    }

    /// Un proveedor LLM simulado para el worker de la bitácora.
    ///
    /// Registra cada [`ChatRequest`] para poder afirmar el material, el
    /// `max_tokens` del reintento y el número de llamadas; devuelve respuestas
    /// por llamada (cola FIFO que cae a `response` al agotarse) y motivos de
    /// finalización por llamada (cola que cae a `None`).
    struct MockJournalLLM {
        pub chat_calls: Arc<Mutex<Vec<ChatRequest>>>,
        pub responses: Arc<Mutex<VecDeque<String>>>,
        pub response: String,
        pub finish_reasons: Arc<Mutex<VecDeque<Option<FinishReason>>>>,
        /// Cuando es `true`, cada llamada falla en el transporte.
        pub fail_transport: bool,
    }

    #[async_trait]
    impl LLMProvider for MockJournalLLM {
        async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, LLMError> {
            self.chat_calls.lock().unwrap().push(request);
            if self.fail_transport {
                return Err(LLMError::HttpError("journal backend down".into()));
            }
            let content = self
                .responses
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| self.response.clone());
            let finish_reason = self.finish_reasons.lock().unwrap().pop_front().flatten();
            Ok(ChatResponse {
                message: ChatMessage {
                    role: "assistant".into(),
                    content,
                    tool_calls: None,
                    tool_result: None,
                    tool_call_id: None,
                },
                usage: Some(TokenUsage {
                    prompt_tokens: 500,
                    completion_tokens: 200,
                    cached_tokens: 10,
                    reasoning_tokens: 0,
                    cost: 0.001,
                }),
                finish_reason,
            })
        }

        async fn chat_stream(
            &self,
            _request: ChatRequest,
        ) -> Result<
            std::pin::Pin<
                Box<
                    dyn tokio_stream::Stream<
                            Item = Result<crate::llm::provider::StreamEvent, LLMError>,
                        > + Send,
                >,
            >,
            LLMError,
        > {
            unimplemented!("chat_stream not used in tests")
        }
    }

    impl MockJournalLLM {
        fn new(response: &str) -> Self {
            Self {
                chat_calls: Arc::new(Mutex::new(Vec::new())),
                responses: Arc::new(Mutex::new(VecDeque::new())),
                response: response.to_string(),
                finish_reasons: Arc::new(Mutex::new(VecDeque::new())),
                fail_transport: false,
            }
        }

        /// Cola de respuestas por llamada (la primera primero). Al agotarse se
        /// devuelve `response`.
        fn responses(mut self, seq: Vec<&str>) -> Self {
            self.responses = Arc::new(Mutex::new(seq.into_iter().map(String::from).collect()));
            self
        }

        /// Cola de motivos de finalización por llamada. Al agotarse, `None`.
        fn finish_reasons(mut self, seq: Vec<Option<FinishReason>>) -> Self {
            self.finish_reasons = Arc::new(Mutex::new(seq.into_iter().collect()));
            self
        }

        fn wrap(self) -> Arc<dyn LLMProvider> {
            Arc::new(self)
        }
    }

    /// Inserta un mensaje con control total sobre `tokens_count` y `created_at`.
    async fn insert_message(
        pool: &SqlitePool,
        role: &str,
        content: &str,
        tokens: usize,
        created_at: &str,
    ) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO messages (id, role, content, tokens_count, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(&id)
        .bind(role)
        .bind(content)
        .bind(tokens as i64)
        .bind(created_at)
        .execute(pool)
        .await
        .expect("Failed to insert test message");
        id
    }

    /// Inserta un hecho del timeline con un instante dado.
    async fn insert_fact(pool: &SqlitePool, timestamp: &str, fact: &str) {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO timeline_events (id, timestamp, category, fact) \
             VALUES (?1, ?2, 'lifestyle', ?3)",
        )
        .bind(&id)
        .bind(timestamp)
        .bind(fact)
        .execute(pool)
        .await
        .expect("Failed to insert test fact");
    }

    /// Una entrada ya escrita, lista para sembrar la bitácora.
    fn entry_for(profile_id: &str, date: &str, content: &str) -> JournalEntry {
        JournalEntry {
            id: uuid::Uuid::new_v4().to_string(),
            profile_id: profile_id.to_string(),
            date: date.to_string(),
            mood: None,
            content: content.to_string(),
            achievements: Vec::new(),
            struggles: Vec::new(),
            tags: Vec::new(),
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    async fn count_messages(pool: &SqlitePool) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM messages")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    async fn count_facts(pool: &SqlitePool) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM timeline_events")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    async fn count_memory(pool: &SqlitePool) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM memory")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    /// Un `Config` con valores por defecto, para el arranque del pool.
    fn test_config() -> Config {
        Config {
            host: "0.0.0.0".into(),
            port: 3000,
            database_url: ":memory:".into(),
            log_level: "debug".into(),
            openrouter_api_key: None,
            openrouter_model: "anthropic/claude-sonnet-20241022".into(),
            openrouter_base_url: "https://openrouter.ai/api/v1".into(),
            ollama_base_url: "http://localhost:11434".into(),
            ollama_model: "llama3.2:3b".into(),
            auth_enabled: false,
            auth_issuer_url: "http://localhost:8080".into(),
            auth_client_id: String::new(),
            auth_client_secret: String::new(),
            auth_redirect_url: "http://localhost:3000/auth/callback".into(),
            auth_post_logout_redirect_url: "http://localhost:3000".into(),
            jwt_secret: String::new(),
            openweather_api_key: None,
            google_places_api_key: None,
            brave_search_api_key: None,
            collapse_threshold_tokens: 2000,
            collapse_model: "mistralai/mistral-small".into(),
            memory_batch_tokens: 2000,
            memory_inactivity_minutes: 30,
            memory_overlap: 2,
            memory_poll_interval_minutes: 30,
            memory_model: "mistralai/mistral-small".into(),
            semantic_model: "mistralai/mistral-small".into(),
            timeline_model: "mistralai/mistral-small".into(),
            journal_model: "mistralai/mistral-small".into(),
            rag_budget_tokens: 2000,
            embedding_provider: None,
            embedding_model: None,
            embedding_dimension: None,
        }
    }

    // ─── 4.1 — escribe la entrada de un día con material ────────────────────

    /// Scenario: Escribe la entrada de un día con material.
    #[tokio::test]
    async fn test_journal_worker_writes_an_entry_for_a_day_with_material() {
        let db = test_db().await;
        put(&db, "timezone", "Europe/Madrid").await;
        let now = madrid_at(2026, 10, 19, 4, 5); // último día cerrado: 2026-10-18
        insert_message(
            &db,
            "user",
            "Hoy fui a correr por la vía verde.",
            100,
            "2026-10-18T09:00:00Z",
        )
        .await;
        insert_fact(&db, "2026-10-18T09:30:00+00:00", "Corrí 8 km por la vía verde").await;

        let mock = MockJournalLLM::new(valid_entry_json());
        let calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        JournalWorker::evaluate(&db, provider, now, TEST_JOURNAL_MODEL).await;

        let profile = ProfilesRepo::get_or_create(&db).await.expect("default profile");
        let entry = JournalRepo::get(&db, &profile.id, "2026-10-18")
            .await
            .expect("read the entry")
            .expect("the worker must have written the entry");
        assert_eq!(entry.date, "2026-10-18", "the entry carries the day's date");
        assert_eq!(entry.content, "El relato del día.", "the entry carries the model's story");
        assert_eq!(entry.mood, Some(Mood::Good), "a valid mood is stored");

        let mark = SettingsRepo::get(&db, "journal_last_judged_day")
            .await
            .unwrap();
        assert_eq!(mark.as_deref(), Some("2026-10-18"), "the day is marked");
        assert_eq!(
            calls.lock().unwrap().len(),
            1,
            "one pass judges one day with one model call"
        );
    }

    // ─── 4.2 — sin material no hay llamada ──────────────────────────────────

    /// Scenario: Sin material no hay llamada ni entrada, pero el día queda juzgado.
    #[tokio::test]
    async fn test_journal_worker_without_material_marks_the_day_without_calling_the_model() {
        let db = test_db().await;
        put(&db, "timezone", "Europe/Madrid").await;
        let now = madrid_at(2026, 10, 19, 4, 5);

        let mock = MockJournalLLM::new(valid_entry_json());
        let calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        JournalWorker::evaluate(&db, provider, now, TEST_JOURNAL_MODEL).await;

        assert!(
            calls.lock().unwrap().is_empty(),
            "sin mensajes ni hechos no se llama al modelo"
        );

        let profile = ProfilesRepo::get_or_create(&db).await.expect("default profile");
        assert!(
            JournalRepo::get(&db, &profile.id, "2026-10-18")
                .await
                .unwrap()
                .is_none(),
            "no entry is written"
        );

        let mark = SettingsRepo::get(&db, "journal_last_judged_day")
            .await
            .unwrap();
        assert_eq!(mark.as_deref(), Some("2026-10-18"), "the day is still marked");
    }

    // ─── 4.3 — la entrada anterior sola no es material ──────────────────────

    /// Scenario: La entrada anterior sola no es material.
    #[tokio::test]
    async fn test_journal_worker_a_previous_entry_alone_is_not_material() {
        let db = test_db().await;
        put(&db, "timezone", "Europe/Madrid").await;
        let now = madrid_at(2026, 10, 19, 4, 5);

        let profile = ProfilesRepo::get_or_create(&db).await.expect("default profile");
        JournalRepo::upsert(&db, &entry_for(&profile.id, "2026-10-17", "Ayer tocó la guitarra."))
            .await
            .expect("previous entry");

        let mock = MockJournalLLM::new(valid_entry_json());
        let calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        JournalWorker::evaluate(&db, provider, now, TEST_JOURNAL_MODEL).await;

        assert!(
            calls.lock().unwrap().is_empty(),
            "la entrada anterior sola no es material: no hay llamada"
        );
        assert!(
            JournalRepo::get(&db, &profile.id, "2026-10-18")
                .await
                .unwrap()
                .is_none(),
            "no entry for the empty day"
        );

        let mark = SettingsRepo::get(&db, "journal_last_judged_day")
            .await
            .unwrap();
        assert_eq!(mark.as_deref(), Some("2026-10-18"), "the empty day is marked");
    }

    // ─── 4.4 — «nada que contar» ────────────────────────────────────────────

    /// Scenario: «Nada que contar» marca el día y no escribe nada.
    #[tokio::test]
    async fn test_journal_worker_nothing_to_tell_marks_the_day_without_writing() {
        let db = test_db().await;
        put(&db, "timezone", "Europe/Madrid").await;
        let now = madrid_at(2026, 10, 19, 4, 5);
        insert_message(&db, "user", "Solo di buenos días y ya.", 100, "2026-10-18T09:00:00Z")
            .await;

        let mock = MockJournalLLM::new(r#"{"write": false}"#);
        let calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        JournalWorker::evaluate(&db, provider, now, TEST_JOURNAL_MODEL).await;

        let profile = ProfilesRepo::get_or_create(&db).await.expect("default profile");
        assert!(
            JournalRepo::get(&db, &profile.id, "2026-10-18")
                .await
                .unwrap()
                .is_none(),
            "write=false: no entry"
        );

        let mark = SettingsRepo::get(&db, "journal_last_judged_day")
            .await
            .unwrap();
        assert_eq!(mark.as_deref(), Some("2026-10-18"), "the day is marked as judged");
        assert_eq!(
            calls.lock().unwrap().len(),
            1,
            "write=false is a valid, definitive answered call"
        );
    }

    // ─── 4.5 — un ánimo fuera del conjunto no invalida el relato ────────────

    /// Scenario: Un ánimo fuera del conjunto no invalida el relato.
    #[tokio::test]
    async fn test_journal_worker_an_out_of_set_mood_and_misshapen_lists_do_not_block_the_story() {
        let db = test_db().await;
        put(&db, "timezone", "Europe/Madrid").await;
        let now = madrid_at(2026, 10, 19, 4, 5);
        insert_message(&db, "user", "Un día intenso de trabajo.", 100, "2026-10-18T09:00:00Z")
            .await;

        let mock = MockJournalLLM::new(
            r#"{"write": true, "content": "El relato del día.", "mood": "genial", "achievements": "no es lista", "struggles": [""], "tags": [42, "ok"]}"#,
        );
        let provider = mock.wrap();

        JournalWorker::evaluate(&db, provider, now, TEST_JOURNAL_MODEL).await;

        let profile = ProfilesRepo::get_or_create(&db).await.expect("default profile");
        let entry = JournalRepo::get(&db, &profile.id, "2026-10-18")
            .await
            .unwrap()
            .expect("the story is still written");
        assert_eq!(entry.content, "El relato del día.");
        assert_eq!(
            entry.mood, None,
            "un ánimo fuera del conjunto se trata como ausente, nunca se persiste tal cual"
        );
        assert_eq!(
            entry.achievements,
            Vec::<String>::new(),
            "un campo que no es una lista es una lista vacía"
        );
        assert_eq!(
            entry.struggles,
            Vec::<String>::new(),
            "un elemento vacío se descarta"
        );
        assert_eq!(
            entry.tags,
            vec!["ok".to_string()],
            "un elemento no textual se descarta; uno textual se queda"
        );
    }

    // ─── 4.6 — un relato en blanco es un fallo de forma ─────────────────────

    /// Scenario: Un relato en blanco es un fallo de forma.
    #[tokio::test]
    async fn test_journal_worker_a_blank_content_is_a_shape_failure_never_a_blank_entry() {
        let db = test_db().await;
        put(&db, "timezone", "Europe/Madrid").await;
        let now = madrid_at(2026, 10, 19, 4, 5);
        insert_message(
            &db,
            "user",
            "Hay mensajes, pero el modelo no escribe nada.",
            100,
            "2026-10-18T09:00:00Z",
        )
        .await;

        let mock = MockJournalLLM::new(r#"{"write": true, "content": "   "}"#);
        let calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        JournalWorker::evaluate(&db, provider, now, TEST_JOURNAL_MODEL).await;

        let profile = ProfilesRepo::get_or_create(&db).await.expect("default profile");
        assert!(
            JournalRepo::get(&db, &profile.id, "2026-10-18")
                .await
                .unwrap()
                .is_none(),
            "never a blank entry"
        );

        let mark = SettingsRepo::get(&db, "journal_last_judged_day")
            .await
            .unwrap();
        assert_eq!(
            mark.as_deref(),
            Some("2026-10-18"),
            "the exhausted shape failure still marks the day"
        );
        assert_eq!(
            calls.lock().unwrap().len(),
            2,
            "a shape failure is retried once"
        );
    }

    // ─── 4.7 — el bloque de material ────────────────────────────────────────

    /// Scenario: El bloque lleva los tres materiales y el día.
    #[tokio::test]
    async fn test_journal_worker_the_material_block_declares_the_day_and_carries_the_three_sources() {
        let db = test_db().await;
        put(&db, "timezone", "Europe/Madrid").await;
        let now = madrid_at(2026, 10, 19, 4, 5);

        let profile = ProfilesRepo::get_or_create(&db).await.expect("default profile");
        JournalRepo::upsert(&db, &entry_for(&profile.id, "2026-10-17", "Ayer toqué la guitarra."))
            .await
            .expect("previous entry");

        insert_fact(&db, "2026-10-18T08:00:00+00:00", "Fui al supermercado").await;
        insert_message(
            &db,
            "user",
            "Primer mensaje del día.",
            100,
            "2026-10-18T08:30:00Z",
        )
        .await;
        insert_message(
            &db,
            "assistant",
            "Segundo mensaje del día.",
            100,
            "2026-10-18T09:30:00Z",
        )
        .await;

        let mock = MockJournalLLM::new(valid_entry_json());
        let calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        JournalWorker::evaluate(&db, provider, now, TEST_JOURNAL_MODEL).await;

        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 1, "one call for the day with material");
        let material = user_turn(&calls[0]);
        assert!(
            material.contains("2026-10-18"),
            "el bloque declara el día que se juzga",
        );
        assert!(
            material.contains("Ayer toqué la guitarra."),
            "el bloque lleva la entrada anterior"
        );
        assert!(
            material.contains("Fui al supermercado"),
            "el bloque lleva los hechos del timeline de ese día"
        );
        let first = material
            .find("Primer mensaje del día.")
            .expect("the first message is present");
        let second = material
            .find("Segundo mensaje del día.")
            .expect("the second message is present");
        assert!(
            first < second,
            "los mensajes viajan en orden cronológico, del más antiguo al más reciente"
        );
    }

    // ─── 4.8 — el tope de tokens se queda con los más recientes ─────────────

    /// Scenario: El tope de tokens se queda con los mensajes más recientes.
    #[tokio::test]
    async fn test_journal_worker_messages_over_the_token_budget_keep_the_most_recent() {
        let db = test_db().await;
        put(&db, "timezone", "Europe/Madrid").await;
        let now = madrid_at(2026, 10, 19, 4, 5);

        // 3 × 800 = 2400 tokens > JOURNAL_MATERIAL_TOKEN_BUDGET (2000): el más
        // antiguo no cabe y se queda fuera.
        insert_message(
            &db,
            "user",
            "mensaje antiguo que sobra",
            800,
            "2026-10-18T08:00:00Z",
        )
        .await;
        insert_message(&db, "user", "mensaje mediano", 800, "2026-10-18T09:00:00Z").await;
        insert_message(&db, "user", "mensaje reciente", 800, "2026-10-18T10:00:00Z").await;

        let mock = MockJournalLLM::new(valid_entry_json());
        let calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        JournalWorker::evaluate(&db, provider, now, TEST_JOURNAL_MODEL).await;

        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        let material = user_turn(&calls[0]);
        assert!(
            material.contains("mensaje reciente"),
            "el mensaje más reciente entra"
        );
        assert!(
            material.contains("mensaje mediano"),
            "el segundo más reciente entra (1600 <= 2000)"
        );
        assert!(
            !material.contains("mensaje antiguo que sobra"),
            "el más antiguo que no cabe se queda fuera"
        );
    }

    // ─── 4.9 — idempotencia y perfil por defecto ────────────────────────────

    /// Scenario: La entrada se escribe con el perfil por defecto y por upsert.
    #[tokio::test]
    async fn test_journal_worker_entries_are_upserted_with_the_default_profile_and_a_marked_day_is_not_rejudged() {
        let db = test_db().await;
        put(&db, "timezone", "Europe/Madrid").await;
        let now = madrid_at(2026, 10, 19, 4, 5);
        insert_message(&db, "user", "Material del día 18.", 100, "2026-10-18T09:00:00Z").await;

        let profile = ProfilesRepo::get_or_create(&db).await.expect("default profile");
        // Una entrada ya escrita a mano para el mismo día: el upsert la
        // reemplaza, no duplica.
        JournalRepo::upsert(&db, &entry_for(&profile.id, "2026-10-18", "Entrada escrita a mano."))
            .await
            .expect("manual entry");

        let mock = MockJournalLLM::new(r#"{"write": true, "content": "La entrada del worker.", "mood": "neutral"}"#);
        let calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        JournalWorker::evaluate(&db, provider.clone(), now, TEST_JOURNAL_MODEL).await;

        let entry = JournalRepo::get(&db, &profile.id, "2026-10-18")
            .await
            .unwrap()
            .expect("one row for the day");
        assert_eq!(
            entry.profile_id, profile.id,
            "la entrada va con el perfil por defecto"
        );
        assert_eq!(entry.content, "La entrada del worker.", "el upsert reemplaza la fila");
        let rows: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM journal_entries WHERE profile_id = ?1 AND date = ?2",
        )
        .bind(&profile.id)
        .bind("2026-10-18")
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(rows, 1, "una sola fila por perfil y día");

        // Juzgar dos veces el mismo día: la marca lo impide.
        JournalWorker::evaluate(&db, provider, now, TEST_JOURNAL_MODEL).await;
        assert_eq!(
            calls.lock().unwrap().len(),
            1,
            "un día marcado no se vuelve a juzgar"
        );
        let rows: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM journal_entries WHERE profile_id = ?1 AND date = ?2",
        )
        .bind(&profile.id)
        .bind("2026-10-18")
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(rows, 1, "no se duplica la entrada");
    }

    // ─── 4.10 — recuperación: un día por vuelta ─────────────────────────────

    /// Scenario: Un parón de varios días se recupera un día por vuelta.
    #[tokio::test]
    async fn test_journal_worker_recovers_one_day_per_pass_after_a_break() {
        let db = test_db().await;
        put(&db, "timezone", "Europe/Madrid").await;
        put(&db, "journal_last_judged_day", "2026-10-15").await;
        let now = madrid_at(2026, 10, 19, 4, 5); // último día cerrado: 2026-10-18

        // Material para los tres días pendientes: 16, 17 y 18.
        insert_message(&db, "user", "Mensaje del 16.", 100, "2026-10-16T09:00:00Z").await;
        insert_message(&db, "user", "Mensaje del 17.", 100, "2026-10-17T09:00:00Z").await;
        insert_message(&db, "user", "Mensaje del 18.", 100, "2026-10-18T09:00:00Z").await;

        let mock = MockJournalLLM::new(valid_entry_json());
        let calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        for _ in 0..4 {
            JournalWorker::evaluate(&db, provider.clone(), now, TEST_JOURNAL_MODEL).await;
        }

        let profile = ProfilesRepo::get_or_create(&db).await.expect("default profile");
        assert_eq!(
            calls.lock().unwrap().len(),
            3,
            "tres vueltas con días pendientes; la cuarta no tiene nada que juzgar"
        );
        assert!(
            JournalRepo::get(&db, &profile.id, "2026-10-16")
                .await
                .unwrap()
                .is_some(),
            "la primera vuelta juzga el 16"
        );
        assert!(
            JournalRepo::get(&db, &profile.id, "2026-10-17")
                .await
                .unwrap()
                .is_some(),
            "la segunda el 17"
        );
        assert!(
            JournalRepo::get(&db, &profile.id, "2026-10-18")
                .await
                .unwrap()
                .is_some(),
            "la tercera el 18"
        );
        let mark = SettingsRepo::get(&db, "journal_last_judged_day")
            .await
            .unwrap();
        assert_eq!(mark.as_deref(), Some("2026-10-18"));
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM journal_entries")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(total, 3, "exactamente una entrada por día juzgado");
    }

    // ─── 4.11 — arranque sin marca ──────────────────────────────────────────

    /// Scenario: Sin marca de progreso, el primer objetivo es el último día cerrado.
    #[tokio::test]
    async fn test_journal_worker_on_first_startup_it_judges_the_last_closed_day_not_the_history() {
        let db = test_db().await;
        put(&db, "timezone", "Europe/Madrid").await;
        let now = madrid_at(2026, 10, 19, 4, 5); // último día cerrado: 2026-10-18

        insert_message(&db, "user", "Mensaje del 17.", 100, "2026-10-17T09:00:00Z").await;
        insert_message(&db, "user", "Mensaje del 18.", 100, "2026-10-18T09:00:00Z").await;

        let mock = MockJournalLLM::new(valid_entry_json());
        let calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        JournalWorker::evaluate(&db, provider, now, TEST_JOURNAL_MODEL).await;

        let profile = ProfilesRepo::get_or_create(&db).await.expect("default profile");
        assert_eq!(
            calls.lock().unwrap().len(),
            1,
            "sin marca, solo se juzga el último día cerrado"
        );
        assert!(
            JournalRepo::get(&db, &profile.id, "2026-10-18")
                .await
                .unwrap()
                .is_some(),
            "la primera vuelta juzga el día cerrado"
        );
        assert!(
            JournalRepo::get(&db, &profile.id, "2026-10-17")
                .await
                .unwrap()
                .is_none(),
            "no recorre la historia hacia atrás"
        );
        let mark = SettingsRepo::get(&db, "journal_last_judged_day")
            .await
            .unwrap();
        assert_eq!(mark.as_deref(), Some("2026-10-18"));
    }

    // ─── 4.12 — apagada no hace nada y al encenderla continúa ───────────────

    /// Scenario: Apagada no hace nada.
    #[tokio::test]
    async fn test_journal_worker_when_off_it_does_nothing_and_resumes_where_it_was() {
        let db = test_db().await;
        put(&db, "timezone", "Europe/Madrid").await;
        put(&db, "journal_enabled", "false").await;
        let now = madrid_at(2026, 10, 19, 4, 5);
        insert_message(&db, "user", "Material del día 18.", 100, "2026-10-18T09:00:00Z").await;

        let mock = MockJournalLLM::new(valid_entry_json());
        let calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        // Apagada: ni llamada, ni entrada, ni marca.
        JournalWorker::evaluate(&db, provider.clone(), now, TEST_JOURNAL_MODEL).await;
        assert!(
            calls.lock().unwrap().is_empty(),
            "apagada: no se llama al modelo"
        );
        let profile = ProfilesRepo::get_or_create(&db).await.expect("default profile");
        assert!(
            JournalRepo::get(&db, &profile.id, "2026-10-18")
                .await
                .unwrap()
                .is_none(),
            "apagada: no escribe ninguna entrada"
        );
        assert!(
            SettingsRepo::get(&db, "journal_last_judged_day")
                .await
                .unwrap()
                .is_none(),
            "apagada: no toca la marca de progreso"
        );

        // Encendida: continúa donde iba (el 18 sigue pendiente).
        put(&db, "journal_enabled", "true").await;
        JournalWorker::evaluate(&db, provider, now, TEST_JOURNAL_MODEL).await;
        assert_eq!(
            calls.lock().unwrap().len(),
            1,
            "al encenderla continúa por donde iba"
        );
        assert!(
            JournalRepo::get(&db, &profile.id, "2026-10-18")
                .await
                .unwrap()
                .is_some(),
            "el día pendiente se juzga al encenderla"
        );
        let mark = SettingsRepo::get(&db, "journal_last_judged_day")
            .await
            .unwrap();
        assert_eq!(mark.as_deref(), Some("2026-10-18"));
    }

    // ─── 5.1 — el reintento y el presupuesto doblado ────────────────────────

    /// Scenario: Un truncamiento provoca un reintento con el doble de presupuesto.
    #[tokio::test]
    async fn test_journal_worker_a_truncated_first_attempt_retries_with_doubled_max_tokens() {
        let db = test_db().await;
        put(&db, "timezone", "Europe/Madrid").await;
        let now = madrid_at(2026, 10, 19, 4, 5);
        insert_message(&db, "user", "Material del día.", 100, "2026-10-18T09:00:00Z").await;

        // El primer intento se corta por presupuesto y no cumple el contrato; el
        // segundo (respaldo) es válido.
        let mock = MockJournalLLM::new(valid_entry_json())
            .responses(vec!["JSON cortado por el presupuesto"])
            .finish_reasons(vec![Some(FinishReason::Length)]);
        let calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        JournalWorker::evaluate(&db, provider, now, TEST_JOURNAL_MODEL).await;

        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 2, "el truncamiento se reintenta una vez");
        let first = calls[0]
            .max_tokens
            .expect("the attempt carries the role's budget");
        let second = calls[1]
            .max_tokens
            .expect("the retry carries a budget");
        assert_eq!(
            second,
            first * 2,
            "un corte por presupuesto dobla el max_tokens del reintento"
        );

        let profile = ProfilesRepo::get_or_create(&db).await.expect("default profile");
        assert!(
            JournalRepo::get(&db, &profile.id, "2026-10-18")
                .await
                .unwrap()
                .is_some(),
            "si el reintento devuelve un contrato válido, la entrada se escribe"
        );
    }

    /// Scenario: Un fallo que no es un truncamiento repite los parámetros.
    #[tokio::test]
    async fn test_journal_worker_a_non_truncation_failure_retries_with_the_same_params() {
        let db = test_db().await;
        put(&db, "timezone", "Europe/Madrid").await;
        let now = madrid_at(2026, 10, 19, 4, 5);
        insert_message(&db, "user", "Material del día.", 100, "2026-10-18T09:00:00Z").await;

        // El primer intento devuelve un contrato inválido sin truncarse.
        let mock = MockJournalLLM::new(valid_entry_json())
            .responses(vec!["esto no es un objeto JSON"])
            .finish_reasons(vec![Some(FinishReason::Stop)]);
        let calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        JournalWorker::evaluate(&db, provider, now, TEST_JOURNAL_MODEL).await;

        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 2, "un fallo de forma también se reintenta una vez");
        assert_eq!(
            calls[1].max_tokens, calls[0].max_tokens,
            "un fallo que no es un truncamiento repite los parámetros, sin doblar"
        );
    }

    // ─── 5.2 — dos fallos dejan la fila honesta y no se reintenta el día ────

    /// Scenario: Dos fallos dejan la fila honesta y no se reintenta el día.
    #[tokio::test]
    async fn test_journal_worker_two_failed_attempts_leave_an_honest_row_per_attempt_and_mark_the_day() {
        let db = test_db().await;
        put(&db, "timezone", "Europe/Madrid").await;
        let now = madrid_at(2026, 10, 19, 4, 5);
        insert_message(&db, "user", "Material del día.", 100, "2026-10-18T09:00:00Z").await;

        let mock = MockJournalLLM::new("esto no es un JSON válido")
            .responses(vec!["esto tampoco"])
            .finish_reasons(vec![Some(FinishReason::Stop), Some(FinishReason::Stop)]);
        let calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        JournalWorker::evaluate(&db, provider.clone(), now, TEST_JOURNAL_MODEL).await;

        // Una fila por intento, honesta: kind = 'journal', status = 'error',
        // perfil nulo, con el motivo de finalización, el uso y un preview.
        let rows = sqlx::query(
            "SELECT status, error_message, prompt_tokens, completion_tokens, profile_id \
             FROM llm_requests WHERE kind = 'journal'",
        )
        .fetch_all(&db)
        .await
        .expect("journal rows exist");
        assert_eq!(rows.len(), 2, "una fila por intento");
        for row in &rows {
            let status: String = row.get(0);
            assert_eq!(status, "error", "ambos intentos fallidos son error");
            let error: Option<String> = row.get(1);
            let error = error.expect("la fila de error lleva su diagnóstico");
            assert!(
                error.contains("shape failure"),
                "el motivo del fallo de forma está en error_message: {error}"
            );
            assert!(
                error.contains("preview"),
                "un preview del contenido está en error_message: {error}"
            );
            assert!(
                error.contains("stop"),
                "el motivo de finalización está en error_message: {error}"
            );
            assert_eq!(row.get::<i64, _>(2), 500, "el uso de prompt se registra");
            assert_eq!(row.get::<i64, _>(3), 200, "el uso de completado se registra");
            assert!(
                Option::<String>::from(row.get(4)).is_none(),
                "profile_id es NULL: es una operación del sistema"
            );
        }

        // El día queda marcado: la vuelta siguiente no vuelve a llamar al modelo.
        let mark = SettingsRepo::get(&db, "journal_last_judged_day")
            .await
            .unwrap();
        assert_eq!(mark.as_deref(), Some("2026-10-18"), "el día agotado queda marcado");
        let profile = ProfilesRepo::get_or_create(&db).await.expect("default profile");
        assert!(
            JournalRepo::get(&db, &profile.id, "2026-10-18")
                .await
                .unwrap()
                .is_none(),
            "el fallo no escribe ninguna entrada"
        );

        JournalWorker::evaluate(&db, provider, now, TEST_JOURNAL_MODEL).await;
        assert_eq!(
            calls.lock().unwrap().len(),
            2,
            "la vuelta siguiente no vuelve a llamar al modelo por ese día"
        );
    }

    // ─── 5.3 — la fila refleja la validación, no el transporte ──────────────

    /// Scenario: Una respuesta que no valida se registra como error.
    #[tokio::test]
    async fn test_journal_worker_a_response_that_fails_validation_is_an_error_row() {
        let db = test_db().await;
        put(&db, "timezone", "Europe/Madrid").await;
        let now = madrid_at(2026, 10, 19, 4, 5);
        insert_message(&db, "user", "Material del día.", 100, "2026-10-18T09:00:00Z").await;

        // El transporte llega bien, pero el contenido no cumple el contrato.
        let provider = MockJournalLLM::new("no es un objeto JSON").wrap();
        JournalWorker::evaluate(&db, provider, now, TEST_JOURNAL_MODEL).await;

        let rows = sqlx::query(
            "SELECT status, error_message, profile_id FROM llm_requests WHERE kind = 'journal'",
        )
        .fetch_all(&db)
        .await
        .expect("journal rows exist");
        assert!(rows.len() >= 1, "el intento (y su reintento) se registran");
        for row in &rows {
            let status: String = row.get(0);
            assert_eq!(
                status, "error",
                "la fila refleja la validación, no el transporte"
            );
            let error: Option<String> = row.get(1);
            assert!(
                error.as_deref().unwrap_or("").contains("shape failure"),
                "el motivo del fallo de validación está en error_message"
            );
            assert!(
                Option::<String>::from(row.get(2)).is_none(),
                "profile_id es NULL"
            );
        }
    }

    /// Scenario: Una llamada correcta se registra como éxito.
    #[tokio::test]
    async fn test_journal_worker_a_valid_response_is_a_success_row_with_null_profile() {
        let db = test_db().await;
        put(&db, "timezone", "Europe/Madrid").await;
        let now = madrid_at(2026, 10, 19, 4, 5);
        insert_message(&db, "user", "Material del día.", 100, "2026-10-18T09:00:00Z").await;

        let provider = MockJournalLLM::new(valid_entry_json()).wrap();
        JournalWorker::evaluate(&db, provider, now, TEST_JOURNAL_MODEL).await;

        assert_eq!(
            CallKind::Journal.as_str(),
            "journal",
            "el origen canónico de la bitácora es 'journal'"
        );

        let rows = sqlx::query(
            "SELECT status, kind, profile_id FROM llm_requests WHERE kind = 'journal'",
        )
        .fetch_all(&db)
        .await
        .expect("journal rows exist");
        assert_eq!(rows.len(), 1, "un solo intento válido es una sola fila");
        let status: String = rows[0].get(0);
        assert_eq!(status, "success", "una respuesta que cumple el contrato es éxito");
        let kind: String = rows[0].get(1);
        assert_eq!(kind, "journal", "la fila lleva kind = 'journal'");
        assert!(
            Option::<String>::from(rows[0].get(2)).is_none(),
            "profile_id es NULL"
        );
    }

    // ─── 5.4 — confinamiento del fallo ──────────────────────────────────────

    /// Scenario: El fallo no toca nada más.
    #[tokio::test]
    async fn test_journal_worker_an_exhausted_failure_touches_nothing_and_the_next_day_writes_normally() {
        let db = test_db().await;
        put(&db, "timezone", "Europe/Madrid").await;
        put(&db, "journal_last_judged_day", "2026-10-15").await;
        let now = madrid_at(2026, 10, 19, 4, 5); // pendientes: 16, 17 y 18

        insert_message(&db, "user", "Material del 16.", 100, "2026-10-16T09:00:00Z").await;
        insert_fact(&db, "2026-10-16T10:00:00+00:00", "Un hecho del 16").await;
        insert_message(&db, "user", "Material del 17.", 100, "2026-10-17T09:00:00Z").await;

        let (messages_before, facts_before) = (count_messages(&db).await, count_facts(&db).await);

        // El día 16 falla agotado: dos intentos inválidos.
        let mock = MockJournalLLM::new(valid_entry_json())
            .responses(vec!["mala respuesta", "otra mala respuesta"]);
        let calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        JournalWorker::evaluate(&db, provider.clone(), now, TEST_JOURNAL_MODEL).await;

        // Confinamiento: no cambia nada salvo la marca.
        assert_eq!(
            count_messages(&db).await,
            messages_before,
            "messages no cambia"
        );
        assert_eq!(count_facts(&db).await, facts_before, "timeline_events no cambia");
        assert_eq!(count_memory(&db).await, 0, "memory no cambia");
        let profile = ProfilesRepo::get_or_create(&db).await.expect("default profile");
        assert!(
            JournalRepo::get(&db, &profile.id, "2026-10-16")
                .await
                .unwrap()
                .is_none(),
            "journal_entries no gana ninguna fila"
        );
        let mark = SettingsRepo::get(&db, "journal_last_judged_day")
            .await
            .unwrap();
        assert_eq!(
            mark.as_deref(),
            Some("2026-10-16"),
            "el fallo agotado marca su día y no entra en bucle"
        );

        // El worker sigue vivo: la vuelta siguiente escribe el 17 con normalidad.
        JournalWorker::evaluate(&db, provider, now, TEST_JOURNAL_MODEL).await;
        assert_eq!(
            calls.lock().unwrap().len(),
            3,
            "dos intentos fallidos para el 16 y una llamada válida para el 17"
        );
        assert!(
            JournalRepo::get(&db, &profile.id, "2026-10-17")
                .await
                .unwrap()
                .is_some(),
            "la vuelta siguiente escribe el día siguiente con normalidad"
        );
        let mark = SettingsRepo::get(&db, "journal_last_judged_day")
            .await
            .unwrap();
        assert_eq!(mark.as_deref(), Some("2026-10-17"));
    }

    // ─── 5.5 — el arranque en el pool ───────────────────────────────────────

    /// Scenario: El arranque del pool arranca el worker.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_pool_starts_and_stops_the_journal_worker_even_without_embeddings() {
        let db = test_db().await;
        let config = test_config();
        let mut pool = WorkerPool::start(db, &config, MockJournalLLM::new(valid_entry_json()).wrap(), None);

        assert!(
            pool.journal.is_some(),
            "el pool arranca y retiene el JournalWorker"
        );
        assert!(
            pool.memory.is_none(),
            "sin proveedor de embeddings, solo el worker episódico se queda sin arrancar"
        );

        pool.shutdown().await;
    }
}
