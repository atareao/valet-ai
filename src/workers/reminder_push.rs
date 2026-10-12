//! Worker que entrega los recordatorios WebPush vencidos.
//!
//! Cada ciclo (60 s) selecciona los `reminders` con `status='pending'` y
//! `notified_at IS NULL`, compara su `datetime` con «ahora» **en Rust** (ver el
//! aviso sobre los formatos de fecha en `openspec/changes/webpush-notifications/
//! design.md`) y entrega el aviso a todas las suscripciones registradas.
//!
//! El transporte se inyecta como [`PushSender`], de modo que el núcleo del
//! worker —selección, ventana, marca— se prueba sin red.
//!
//! Ver `openspec/changes/webpush-notifications/specs/workers/spec.md`.

use std::time::Duration;

use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use crate::db::repos::push::PushRepo;
use crate::db::repos::reminders::RemindersRepo;
use crate::db::repos::settings::SettingsRepo;
use crate::services::push::{self, DeliveryOutcome, PushPayload, PushSender};

/// Clave de `settings` con la ventana de gracia, en horas.
pub const GRACE_HOURS_SETTING: &str = "push_reminder_grace_hours";

/// Ventana de gracia por defecto cuando el ajuste falta o no parsea.
pub const DEFAULT_GRACE_HOURS: i64 = 24;

/// Cadencia del worker, en segundos.
pub const TICK_INTERVAL_SECS: u64 = 60;

/// Resumen de un ciclo del worker.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TickOutcome {
    /// Recordatorios entregados con éxito (y marcados) en este ciclo.
    pub delivered: usize,
    /// Recordatorios descartados por caer fuera de la ventana de gracia.
    pub skipped_old: usize,
    /// Suscripciones depuradas porque el servicio respondió `Gone`.
    pub pruned: usize,
    /// Recordatorios cuyo `datetime` no parseó: ni se envían ni se marcan.
    pub unparseable: usize,
}

/// Entrega los recordatorios vencidos que corresponden a este ciclo.
///
/// Función pura respecto al reloj: recibe `now` por parámetro. Selecciona en SQL
/// los recordatorios pendientes sin notificar (**sin** filtrar por fecha en
/// SQL, porque `reminders.datetime` está en RFC 3339 y `datetime('now')` de
/// SQLite lleva espacio: la comparación de cadenas sería silenciosamente
/// incorrecta) y decide en Rust.
pub async fn deliver_due_reminders(
    pool: &SqlitePool,
    sender: &dyn PushSender,
    now: DateTime<Utc>,
    grace_hours: i64,
) -> Result<TickOutcome, sqlx::Error> {
    let mut outcome = TickOutcome::default();

    // Sin filtro de fecha en SQL a propósito: la comparación se hace en Rust.
    let rows = sqlx::query(
        "SELECT id, text, datetime FROM reminders
         WHERE status = 'pending' AND notified_at IS NULL",
    )
    .fetch_all(pool)
    .await?;

    // Las suscripciones se leen una vez por ciclo; las que respondan `Gone` se
    // van retirando de la lista para no reenviarles en el mismo ciclo.
    let mut subscriptions = PushRepo::list_all(pool).await?;

    for row in rows {
        let id: String = row.get("id");
        let text: String = row.get("text");
        let datetime: String = row.get("datetime");

        let due = match DateTime::parse_from_rfc3339(&datetime) {
            Ok(parsed) => parsed.with_timezone(&Utc),
            Err(error) => {
                // No se marca: un `datetime` corrupto no debe consumir el aviso.
                outcome.unparseable += 1;
                tracing::warn!(
                    reminder_id = %id,
                    datetime = %datetime,
                    error = %error,
                    "reminder_push: datetime no parseable; se omite sin marcar"
                );
                continue;
            }
        };

        // Futuro: no se toca.
        if due > now {
            continue;
        }

        // Fuera de la ventana de gracia: se descarta sin enviar, pero se marca
        // para no arrastrarlo indefinidamente.
        if (now - due) > chrono::Duration::hours(grace_hours) {
            RemindersRepo::mark_notified(pool, &id).await?;
            outcome.skipped_old += 1;
            tracing::warn!(
                reminder_id = %id,
                grace_hours = %grace_hours,
                "reminder_push: recordatorio fuera de la ventana de gracia; descartado sin enviar"
            );
            continue;
        }

        // Sin dispositivos suscritos no hay envío ni marca: se entregará cuando
        // aparezca uno, si sigue dentro de la ventana.
        if subscriptions.is_empty() {
            continue;
        }

        let payload = PushPayload {
            title: "Recordatorio".to_string(),
            body: text,
            icon: None,
            url: Some("/".to_string()),
            tag: Some(format!("reminder-{id}")),
        };

        let mut delivered_any = false;
        let mut gone_endpoints: Vec<String> = Vec::new();

        for sub in &subscriptions {
            match sender.send(sub, &payload).await {
                Ok(DeliveryOutcome::Delivered) => {
                    delivered_any = true;
                }
                Ok(DeliveryOutcome::Gone) => {
                    outcome.pruned += 1;
                    gone_endpoints.push(sub.endpoint.clone());
                    tracing::warn!(
                        endpoint = %sub.endpoint,
                        "reminder_push: suscripción caducada; se elimina"
                    );
                }
                Ok(DeliveryOutcome::Failed) => {
                    tracing::warn!(
                        endpoint = %sub.endpoint,
                        "reminder_push: fallo de envío; la suscripción se conserva"
                    );
                }
                Err(error) => {
                    tracing::warn!(
                        endpoint = %sub.endpoint,
                        error = %error,
                        "reminder_push: error al enviar; la suscripción se conserva"
                    );
                }
            }
        }

        for endpoint in &gone_endpoints {
            PushRepo::delete_by_endpoint(pool, endpoint).await?;
        }
        subscriptions.retain(|sub| !gone_endpoints.contains(&sub.endpoint));

        // Entrega como máximo una vez: solo se marca si al menos una entrega fue
        // correcta.
        if delivered_any {
            RemindersRepo::mark_notified(pool, &id).await?;
            outcome.delivered += 1;
        }
    }

    Ok(outcome)
}

/// Bucle del worker: entrega los recordatorios vencidos cada
/// [`TICK_INTERVAL_SECS`] segundos, o hasta que llegue la señal de `shutdown`.
pub struct ReminderPushWorker;

impl ReminderPushWorker {
    /// Arranca el worker en una tarea de tokio.
    pub fn start(db: SqlitePool, mut shutdown_rx: broadcast::Receiver<()>) -> JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(TICK_INTERVAL_SECS));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        Self::tick(&db).await;
                    }
                    _ = shutdown_rx.recv() => {
                        tracing::info!("ReminderPushWorker shutting down");
                        break;
                    }
                }
            }
        })
    }

    /// Un ciclo: lee la ventana, carga las claves VAPID y entrega.
    async fn tick(db: &SqlitePool) {
        let grace_hours = read_grace_hours(db).await;

        let keys = match push::load_or_generate(db).await {
            Ok(keys) => keys,
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    "reminder_push: no se pudieron cargar las claves VAPID; se salta el ciclo"
                );
                return;
            }
        };
        let client = match push::client() {
            Ok(client) => client,
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    "reminder_push: no se pudo construir el cliente HTTP; se salta el ciclo"
                );
                return;
            }
        };

        let sender = push::WebPushSender::new(client, keys);
        match deliver_due_reminders(db, &sender, Utc::now(), grace_hours).await {
            Ok(outcome) => {
                if outcome.delivered > 0 || outcome.skipped_old > 0 || outcome.pruned > 0 {
                    tracing::info!(
                        delivered = %outcome.delivered,
                        skipped_old = %outcome.skipped_old,
                        pruned = %outcome.pruned,
                        "reminder_push: ciclo completado"
                    );
                }
            }
            Err(error) => {
                tracing::error!(error = %error, "reminder_push: fallo el ciclo");
            }
        }
    }
}

/// Lee `push_reminder_grace_hours` de `settings`; por defecto
/// [`DEFAULT_GRACE_HOURS`] si falta, está vacío o no parsea.
async fn read_grace_hours(pool: &SqlitePool) -> i64 {
    match SettingsRepo::get(pool, GRACE_HOURS_SETTING).await {
        Ok(Some(value)) => value.trim().parse::<i64>().unwrap_or(DEFAULT_GRACE_HOURS),
        _ => DEFAULT_GRACE_HOURS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::collections::HashMap;
    use std::sync::Mutex;

    use crate::db::repos::reminders::Reminder;
    use crate::services::push::PushError;

    async fn setup() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await
            .expect("in-memory sqlite");
        crate::db::schema::run_migrations(&pool)
            .await
            .expect("migrations");
        sqlx::query("INSERT OR IGNORE INTO profiles (id, name) VALUES ('default', 'Default')")
            .execute(&pool)
            .await
            .expect("default profile");
        pool
    }

    async fn add_reminder(pool: &SqlitePool, id: &str, text: &str, datetime: &str) {
        let reminder = Reminder {
            id: id.to_string(),
            profile_id: "default".to_string(),
            text: text.to_string(),
            datetime: datetime.to_string(),
            status: "pending".to_string(),
            created_at: "2026-10-12T00:00:00Z".to_string(),
        };
        RemindersRepo::create(pool, &reminder)
            .await
            .expect("insert reminder");
    }

    async fn add_subscription(pool: &SqlitePool, endpoint: &str) {
        PushRepo::upsert(pool, endpoint, "p256dh", "auth", None)
            .await
            .expect("upsert subscription");
    }

    async fn notified_at(pool: &SqlitePool, id: &str) -> Option<String> {
        sqlx::query_scalar::<_, Option<String>>("SELECT notified_at FROM reminders WHERE id = ?1")
            .bind(id)
            .fetch_one(pool)
            .await
            .expect("read notified_at")
    }

    /// Instantánea fija para todos los tests: `2026-10-12T12:00:00Z`.
    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-10-12T12:00:00Z")
            .expect("fixed instant")
            .with_timezone(&Utc)
    }

    #[derive(Default)]
    struct FakeState {
        calls: Vec<(String, PushPayload)>,
        outcomes: HashMap<String, DeliveryOutcome>,
        error: bool,
    }

    /// `PushSender` de prueba: registra las llamadas y devuelve el resultado
    /// configurado por endpoint (`Delivered` por defecto).
    struct FakeSender {
        state: Mutex<FakeState>,
    }

    impl FakeSender {
        fn new() -> Self {
            Self {
                state: Mutex::new(FakeState::default()),
            }
        }

        fn with_outcome(endpoint: &str, outcome: DeliveryOutcome) -> Self {
            let sender = Self::new();
            sender
                .state
                .lock()
                .expect("lock")
                .outcomes
                .insert(endpoint.to_string(), outcome);
            sender
        }

        fn failing() -> Self {
            let sender = Self::new();
            sender.state.lock().expect("lock").error = true;
            sender
        }

        fn calls(&self) -> Vec<(String, PushPayload)> {
            self.state.lock().expect("lock").calls.clone()
        }
    }

    #[async_trait]
    impl PushSender for FakeSender {
        async fn send(
            &self,
            sub: &crate::db::repos::push::PushSubscription,
            payload: &PushPayload,
        ) -> Result<DeliveryOutcome, PushError> {
            let mut state = self.state.lock().expect("lock");
            state.calls.push((sub.endpoint.clone(), payload.clone()));
            if state.error {
                return Err(PushError::InvalidEndpoint);
            }
            Ok(*state
                .outcomes
                .get(&sub.endpoint)
                .unwrap_or(&DeliveryOutcome::Delivered))
        }
    }

    /// Scenario: un recordatorio vencido se envía y se marca; el ciclo
    /// siguiente no lo reenvía; uno futuro no se toca.
    #[tokio::test]
    async fn due_reminder_is_delivered_once_and_future_untouched() {
        let pool = setup().await;
        add_subscription(&pool, "https://example.invalid/push/1").await;
        add_reminder(&pool, "due", "Comprar leche", "2026-10-12T11:00:00Z").await;
        add_reminder(&pool, "future", "Cita", "2026-10-12T13:00:00Z").await;

        let sender = FakeSender::new();
        let first = deliver_due_reminders(&pool, &sender, now(), 24)
            .await
            .expect("first tick");

        assert_eq!(first.delivered, 1, "the overdue reminder must be delivered");
        assert_eq!(sender.calls().len(), 1, "exactly one send");
        assert!(
            notified_at(&pool, "due").await.is_some(),
            "the delivered reminder must be marked"
        );
        assert!(
            notified_at(&pool, "future").await.is_none(),
            "a future reminder must not be touched"
        );

        let second = deliver_due_reminders(&pool, &sender, now(), 24)
            .await
            .expect("second tick");
        assert_eq!(second.delivered, 0, "nothing delivered on the second tick");
        assert_eq!(
            sender.calls().len(),
            1,
            "the marked reminder must not be re-sent"
        );
    }

    /// Scenario: sin suscripciones no hay envío ni marca; al aparecer una y
    /// seguir dentro de la ventana, se entrega en el ciclo siguiente.
    #[tokio::test]
    async fn no_subscriptions_then_delivered_when_subscribed() {
        let pool = setup().await;
        add_reminder(&pool, "due", "Comprar leche", "2026-10-12T11:00:00Z").await;

        let sender = FakeSender::new();
        let first = deliver_due_reminders(&pool, &sender, now(), 24)
            .await
            .expect("tick without subscriptions");

        assert_eq!(first.delivered, 0);
        assert!(sender.calls().is_empty(), "no subscription, no send");
        assert!(
            notified_at(&pool, "due").await.is_none(),
            "without subscriptions the reminder must NOT be consumed"
        );

        add_subscription(&pool, "https://example.invalid/push/1").await;
        let second = deliver_due_reminders(&pool, &sender, now(), 24)
            .await
            .expect("tick after subscribing");

        assert_eq!(second.delivered, 1, "the still-in-window reminder delivers");
        assert!(notified_at(&pool, "due").await.is_some());
    }

    /// Scenario: vencido fuera de la ventana de gracia se marca sin enviar.
    #[tokio::test]
    async fn overdue_beyond_grace_is_marked_but_not_sent() {
        let pool = setup().await;
        add_subscription(&pool, "https://example.invalid/push/1").await;
        // 30 h before `now`: outside the 24 h window.
        add_reminder(&pool, "old", "Antiguo", "2026-10-11T06:00:00Z").await;

        let sender = FakeSender::new();
        let outcome = deliver_due_reminders(&pool, &sender, now(), 24)
            .await
            .expect("tick");

        assert_eq!(outcome.skipped_old, 1, "one reminder out of the window");
        assert_eq!(outcome.delivered, 0);
        assert!(sender.calls().is_empty(), "nothing is sent for an old one");
        assert!(
            notified_at(&pool, "old").await.is_some(),
            "an out-of-window reminder must be marked"
        );
    }

    /// Scenario: con `grace_hours = 0` todo lo vencido queda fuera de ventana.
    #[tokio::test]
    async fn grace_zero_marks_everything_overdue() {
        let pool = setup().await;
        add_subscription(&pool, "https://example.invalid/push/1").await;
        add_reminder(&pool, "due", "Aviso", "2026-10-12T11:59:00Z").await;

        let sender = FakeSender::new();
        let outcome = deliver_due_reminders(&pool, &sender, now(), 0)
            .await
            .expect("tick");

        assert_eq!(outcome.skipped_old, 1);
        assert_eq!(outcome.delivered, 0);
        assert!(sender.calls().is_empty());
        assert!(notified_at(&pool, "due").await.is_some());
    }

    /// Scenario: el payload identifica el recordatorio.
    #[tokio::test]
    async fn payload_identifies_the_reminder() {
        let pool = setup().await;
        add_subscription(&pool, "https://example.invalid/push/1").await;
        add_reminder(&pool, "rem-42", "Comprar leche", "2026-10-12T11:00:00Z").await;

        let sender = FakeSender::new();
        deliver_due_reminders(&pool, &sender, now(), 24)
            .await
            .expect("tick");

        let calls = sender.calls();
        assert_eq!(calls.len(), 1);
        let (_, payload) = &calls[0];
        assert_eq!(payload.title, "Recordatorio");
        assert_eq!(payload.body, "Comprar leche");
        assert_eq!(payload.url.as_deref(), Some("/"));
        assert_eq!(payload.tag.as_deref(), Some("reminder-rem-42"));
    }

    /// Scenario: `Gone` borra la suscripción y las demás siguen recibiendo;
    /// `Failed` conserva la suscripción.
    #[tokio::test]
    async fn gone_prunes_only_that_subscription_failed_keeps_it() {
        let pool = setup().await;
        add_subscription(&pool, "https://example.invalid/push/gone").await;
        add_subscription(&pool, "https://example.invalid/push/ok").await;
        add_reminder(&pool, "due", "Aviso", "2026-10-12T11:00:00Z").await;

        let sender =
            FakeSender::with_outcome("https://example.invalid/push/gone", DeliveryOutcome::Gone);
        let outcome = deliver_due_reminders(&pool, &sender, now(), 24)
            .await
            .expect("tick");

        assert_eq!(outcome.pruned, 1, "the Gone subscription must be pruned");
        assert_eq!(outcome.delivered, 1, "the healthy one still receives");
        assert!(notified_at(&pool, "due").await.is_some());

        let remaining = PushRepo::list_all(&pool).await.expect("list");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].endpoint, "https://example.invalid/push/ok");

        // A `Failed` outcome keeps the subscription and does not mark.
        let pool2 = setup().await;
        add_subscription(&pool2, "https://example.invalid/push/flaky").await;
        add_reminder(&pool2, "due", "Aviso", "2026-10-12T11:00:00Z").await;
        let failing = FakeSender::with_outcome(
            "https://example.invalid/push/flaky",
            DeliveryOutcome::Failed,
        );
        let outcome2 = deliver_due_reminders(&pool2, &failing, now(), 24)
            .await
            .expect("tick");

        assert_eq!(outcome2.delivered, 0);
        assert_eq!(outcome2.pruned, 0);
        assert_eq!(PushRepo::list_all(&pool2).await.expect("list").len(), 1);
        assert!(
            notified_at(&pool2, "due").await.is_none(),
            "a Failed delivery must not consume the reminder"
        );
    }

    /// Scenario: los formatos `Z` y `+00:00` (snoozeado, con fracción) se
    /// comparan correctamente. Es el test que protege el aviso crítico: en SQL
    /// `'T' > ' '` haría pasar por «futuro» todo lo vencido.
    #[tokio::test]
    async fn z_and_offset_datetimes_compare_correctly() {
        let pool = setup().await;
        add_subscription(&pool, "https://example.invalid/push/1").await;
        add_reminder(&pool, "z", "Con Z", "2026-10-12T11:00:00Z").await;
        add_reminder(
            &pool,
            "snoozed",
            "Snoozeado",
            "2026-10-12T11:30:00.123+00:00",
        )
        .await;

        let sender = FakeSender::new();
        let outcome = deliver_due_reminders(&pool, &sender, now(), 24)
            .await
            .expect("tick");

        assert_eq!(outcome.delivered, 2, "both overdue forms must be delivered");
        assert!(notified_at(&pool, "z").await.is_some());
        assert!(notified_at(&pool, "snoozed").await.is_some());
    }

    /// Scenario: el sender falso recibe las llamadas aunque no haya VAPID
    /// configurado (los tests no necesitan claves).
    #[tokio::test]
    async fn sender_records_calls_without_vapid_configured() {
        let pool = setup().await;
        // The migration seeds the VAPID keys empty: no pair is configured.
        assert_eq!(
            SettingsRepo::get(&pool, "push_vapid_private_key")
                .await
                .unwrap(),
            Some(String::new())
        );
        add_subscription(&pool, "https://example.invalid/push/1").await;
        add_reminder(&pool, "due", "Aviso", "2026-10-12T11:00:00Z").await;

        let sender = FakeSender::new();
        deliver_due_reminders(&pool, &sender, now(), 24)
            .await
            .expect("tick");

        assert_eq!(sender.calls().len(), 1, "the fake sender records the call");
    }

    /// Scenario: el worker no se cae si el sender devuelve error.
    #[tokio::test]
    async fn sender_error_does_not_crash_the_tick() {
        let pool = setup().await;
        add_subscription(&pool, "https://example.invalid/push/1").await;
        add_reminder(&pool, "due", "Aviso", "2026-10-12T11:00:00Z").await;

        let sender = FakeSender::failing();
        let outcome = deliver_due_reminders(&pool, &sender, now(), 24)
            .await
            .expect("a sender error must not fail the tick");

        assert_eq!(outcome.delivered, 0);
        assert_eq!(sender.calls().len(), 1, "the call is still recorded");
        assert!(
            notified_at(&pool, "due").await.is_none(),
            "no delivery means no mark"
        );
    }

    /// Scenario: un `datetime` que no parsea se registra y no se marca.
    #[tokio::test]
    async fn unparseable_datetime_is_skipped_without_marking() {
        let pool = setup().await;
        add_subscription(&pool, "https://example.invalid/push/1").await;
        add_reminder(&pool, "broken", "Roto", "no-es-una-fecha").await;

        let sender = FakeSender::new();
        let outcome = deliver_due_reminders(&pool, &sender, now(), 24)
            .await
            .expect("tick");

        assert_eq!(outcome.unparseable, 1);
        assert_eq!(outcome.delivered, 0);
        assert!(sender.calls().is_empty());
        assert!(notified_at(&pool, "broken").await.is_none());
    }
}
