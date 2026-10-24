//! Bus de eventos del chat y cerrojo de turno único.
//!
//! El chat de Valet es un hilo global: un mensaje escrito en un dispositivo
//! tiene que aparecer en los demás sin recargar (change `chat-cross-device-sync`).
//! Este módulo reúne las dos piezas compartidas que hacen posible esa
//! difusión:
//!
//! * [`ChatEvent`] y el bus `tokio::sync::broadcast` por el que circula
//!   (design D1). El bus transporta **solo** los eventos del chat —mensajes
//!   persistidos e inicio/fin de turno—, nunca fragmentos de texto del turno
//!   (design D2). Los eventos `snapshot` y `resync` son propios del endpoint
//!   `GET /api/chat/events` y no se difunden.
//! * [`TurnLock`], el cerrojo que admite un único turno a la vez (design D6).
//!
//! Ambos viven en [`crate::AppState`] para que los tests puedan inspeccionarlos
//! sin arrancar el servidor.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::db::repos::messages::MessagesCursor;
use crate::models::Message;

/// Capacidad de la cola del bus de eventos del chat (design D1).
///
/// Un receptor que no consume a esta velocidad pierde eventos
/// (`broadcast::error::RecvError::Lagged`); el endpoint lo traduce en un aviso
/// `resync` y cierra el stream, en vez de descartar eventos en silencio.
pub const CHAT_EVENT_BUS_CAPACITY: usize = 256;

/// Intervalo de latido por defecto del stream de eventos, en segundos
/// (design D7). Los proxies intermedios (Traefik, nginx) cierran las conexiones
/// inactivas, así que el stream se mantiene vivo con un comentario SSE.
pub const DEFAULT_CHAT_EVENTS_HEARTBEAT_SECS: u64 = 15;

/// Evento del chat difundido a todos los suscriptores.
///
/// Se serializa con un campo `type` que lo discrimina, igual que
/// [`crate::orchestrator::agent::SSEEvent`], y viaja como un único objeto JSON
/// en la línea `data:` del stream de eventos.
// `MessageCreated` lleva una fila entera y empequeñece a las dos variantes
// unitarias, así que clippy propone boxearla. Se deja sin boxear a propósito: el
// evento solo vive en el anillo del bus y en el stream de cada conexión, donde
// sus ~336 bytes son irrelevantes, y el `Box` añadiría una reserva en el montón
// por cada evento publicado y por cada suscriptor —justo en el camino caliente
// y justo para la variante más grande—.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ChatEvent {
    /// Un mensaje acaba de persistirse: lleva la **fila completa** y su cursor
    /// opaco, de modo que el cliente haga un upsert idempotente por `id`
    /// (design D2).
    #[serde(rename = "message_created")]
    MessageCreated { message: Message, cursor: String },

    /// Un turno ha empezado.
    #[serde(rename = "turn_started")]
    TurnStarted,

    /// Un turno ha terminado, con éxito o con error.
    #[serde(rename = "turn_finished")]
    TurnFinished,
}

impl ChatEvent {
    /// Evento `message_created` de una fila ya persistida.
    ///
    /// El cursor es el de la pareja `(created_at, id)` que define el orden
    /// total de la tabla: es opaco y el cliente solo tiene que devolverlo tal
    /// cual en el siguiente `?since=`.
    pub fn message_created(message: &Message) -> Self {
        Self::MessageCreated {
            cursor: MessagesCursor::from_message(message).encode(),
            message: message.clone(),
        }
    }

    /// Serializa el evento a la cadena JSON de una línea `data:`.
    ///
    /// El respaldo es `resync` —y no un evento de error— porque un evento
    /// ilegible significa que el cliente ha perdido la sincronía y debe releer
    /// con su último cursor. En la práctica no se alcanza: [`Message`] solo
    /// contiene tipos serializables sin pérdida.
    pub fn to_json_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{\"type\":\"resync\"}".to_string())
    }
}

/// Cerrojo del turno único del chat (design D6).
///
/// El turno se adquiere **sin bloquear**: si ya hay uno en curso,
/// [`TurnLock::try_acquire`] devuelve `None` y quien llama responde `409` en vez
/// de esperar (se pidió aviso, no cola).
///
/// Al adquirirlo se publica [`ChatEvent::TurnStarted`] y la guarda devuelta lo
/// libera en su `Drop`, publicando [`ChatEvent::TurnFinished`]. Eso garantiza
/// que un error —o un pánico dentro del turno— no deje el chat bloqueado para
/// siempre.
pub struct TurnLock {
    busy: AtomicBool,
    events: broadcast::Sender<ChatEvent>,
}

impl TurnLock {
    /// Crea el cerrojo, ligado al bus por el que publica el inicio y el fin de
    /// cada turno.
    pub fn new(events: broadcast::Sender<ChatEvent>) -> Self {
        Self {
            busy: AtomicBool::new(false),
            events,
        }
    }

    /// `true` mientras hay un turno en curso.
    pub fn is_busy(&self) -> bool {
        self.busy.load(Ordering::SeqCst)
    }

    /// Intenta adquirir el turno sin bloquear.
    ///
    /// Devuelve la guarda que lo libera —y que publica el fin del turno— o
    /// `None` si ya hay un turno en curso. Al adquirirlo se publica el inicio
    /// del turno.
    pub fn try_acquire(self: &Arc<Self>) -> Option<TurnGuard> {
        if self
            .busy
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return None;
        }
        // Un fallo de envío solo significa que no hay suscriptores.
        let _ = self.events.send(ChatEvent::TurnStarted);
        Some(TurnGuard {
            lock: Arc::clone(self),
        })
    }
}

/// Guarda RAII del turno en curso: libera el cerrojo y publica el fin del turno
/// al soltarse, tanto por el camino feliz como si el turno falla o entra en
/// pánico.
pub struct TurnGuard {
    lock: Arc<TurnLock>,
}

impl Drop for TurnGuard {
    fn drop(&mut self) {
        self.lock.busy.store(false, Ordering::SeqCst);
        let _ = self.lock.events.send(ChatEvent::TurnFinished);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bus() -> (broadcast::Sender<ChatEvent>, broadcast::Receiver<ChatEvent>) {
        broadcast::channel(CHAT_EVENT_BUS_CAPACITY)
    }

    /// El turno es único: mientras la guarda vive, un segundo intento no lo
    /// consigue; al soltarla, sí.
    #[tokio::test]
    async fn test_the_lock_admits_a_single_turn_at_a_time() {
        let (events, mut rx) = bus();
        let lock = Arc::new(TurnLock::new(events));

        let guard = lock.try_acquire().expect("the lock starts free");
        assert!(lock.is_busy());
        assert!(
            lock.try_acquire().is_none(),
            "a second turn must not be admitted while one is running"
        );

        drop(guard);
        assert!(!lock.is_busy(), "dropping the guard must release the lock");
        assert!(
            lock.try_acquire().is_some(),
            "the next turn must be admitted once the previous one finished"
        );

        // Start and finish are published around the turn.
        assert!(matches!(rx.try_recv(), Ok(ChatEvent::TurnStarted)));
        assert!(matches!(rx.try_recv(), Ok(ChatEvent::TurnFinished)));
    }

    /// Un evento `message_created` lleva la fila y el cursor opaco de su pareja
    /// `(created_at, id)`.
    #[test]
    fn test_message_created_carries_the_row_and_its_opaque_cursor() {
        let message = Message {
            id: "m1".to_string(),
            role: "user".to_string(),
            content: "Hola".to_string(),
            tool_calls: None,
            tool_results: None,
            tokens_count: 1,
            collapsed_content: None,
            collapsed_tokens_count: 0,
            is_indexed: false,
            summary_ref: None,
            location: None,
            tools_used: None,
            widgets: None,
            created_at: "2026-10-24 10:00:01".to_string(),
        };

        let json = ChatEvent::message_created(&message).to_json_string();
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");

        assert_eq!(parsed["type"], "message_created");
        assert_eq!(parsed["message"]["id"], "m1");
        assert_eq!(parsed["message"]["content"], "Hola");
        assert_eq!(parsed["cursor"], "2026-10-24 10:00:01|m1");
        assert_eq!(
            MessagesCursor::decode(parsed["cursor"].as_str().expect("cursor"))
                .expect("the cursor must be decodable"),
            MessagesCursor::new("2026-10-24 10:00:01", "m1")
        );
    }
}
