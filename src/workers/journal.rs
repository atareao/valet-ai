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
    use crate::db::repos::settings::SettingsRepo;
    use crate::db::schema::run_migrations;
    use chrono::TimeZone;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    /// Una base en memoria con las migraciones aplicadas.
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
}
