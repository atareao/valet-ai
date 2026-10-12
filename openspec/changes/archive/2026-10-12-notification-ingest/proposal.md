# Proposal: Puerta de entrada para notificaciones externas (ingesta autenticada por aplicación)

## Why

Valet no tiene puerta de entrada para que sistemas externos le avisen. Hoy el usuario se entera de un
formulario de contacto en su web, de que Uptime Kuma ha detectado un servicio caído o de que un
workflow de GitHub Actions ha fallado solo si mira cada herramienta por separado. El transporte
WebPush ya existe y está verificado en producción, pero **su único productor son los recordatorios**.
Este change abre la puerta: un endpoint HTTP autenticado que recibe un mensaje, lo persiste y dispara
el WebPush que ya funciona. Es el segundo productor de WebPush del proyecto.

## What Changes

- **`POST /api/notifications` SHALL autenticar con un token por aplicación** (no con la sesión OIDC),
  validar y normalizar el cuerpo, persistir el mensaje y responder `201` **antes** de despachar el
  WebPush en segundo plano.
- **La autenticación para máquinas SHALL usar una tabla `ingest_keys`** con una clave por aplicación,
  guardando **solo el SHA-256 del token** —el token se muestra una sola vez—, revocable por separado
  y con `last_used_at`; más sus endpoints de gestión con sesión.
- **El middleware de sesión SHALL dejar de ser universal:** `POST /api/notifications` queda exento y
  pasa a exigir token de ingesta. La exención SHALL ser por **par (método, ruta)**, de modo que
  `GET /api/notifications/unread` y `POST /api/notifications/mark-read` sigan exigiendo sesión.
- **`GET /api/notifications/unread` SHALL devolver las no leídas, paginadas y más recientes primero**,
  y **`POST /api/notifications/mark-read` SHALL ser idempotente**, ambos con sesión.
- **El envío masivo SHALL ser una operación del transporte** (`broadcast` en `services::push`), con
  contadores, depurando las suscripciones caducadas; y **la redacción del endpoint en los registros
  SHALL pasar a ser compartida** (el ayudante de host sale del worker a `services::push`).
- **SHALL existir una herramienta del agente `notifications`** con las operaciones `list_unread` y
  `mark_read`, con su registro en el catálogo y su sitio en la skill `pendientes`.
- **SHALL añadirse una sección en la pestaña API Keys del frontend** para crear y revocar claves de
  ingesta.

## Capabilities

### New Capabilities

- `notifications` (**ADDED**): la ingesta autenticada, la persistencia del mensaje y su consumo
  (lectura de no leídas y marcado). No existía ninguna capacidad para notificaciones entrantes; el
  WebPush entregado ya vive en `push`, pero nada cubría la entrada.
- `ingest-keys` (**ADDED**): el token por aplicación para máquinas —colección con nombre, alta, baja,
  `last_used_at` y su verificación—. Es una capacidad nueva porque la autenticación existente
  (`oidc-auth`) es de sesión humana y no cubre credenciales de máquina.

### Modified Capabilities

- `push` (**ADDED** sobre la capacidad existente): se añade el **envío masivo** como operación del
  transporte y se especifica como `## ADDED Requirements` para no colisionar con los headers ya
  presentes en `openspec/specs/push/spec.md`. El transporte no cambia de semántica; se le añade una
  operación que antes vivía embebida en el worker.
- `oidc-auth` (**MODIFIED**): el requirement de enforcement pasa de exención por prefijo de ruta a
  **exención por par (método, ruta)**, porque eximir todo `/api/notifications` dejaría sin sesión la
  lectura y el marcado.
- `frontend` (**ADDED**): la pestaña API Keys gana una sección de claves de ingesta.
- `tools/registry` (**MODIFIED**): el catálogo pasa de 28 a 29 herramientas con la nueva
  `notifications`.
- `orchestrator/skill-router` (**MODIFIED**): la skill `pendientes` cubre ahora también
  `notifications`; el catálogo de skills sigue siendo cerrado (diez).

## Impact

- **Backend:** `src/routes/` (router de notificaciones y de claves), `src/middleware/auth.rs`
  (exención por par método/ruta), `src/db/repos/` (notificaciones y claves de ingesta),
  `src/services/push.rs` (`broadcast` y el ayudante de host compartido), `src/workers/reminder_push.rs`
  (solo por la mudanza del ayudante), `src/tools/` (la herramienta) y `src/orchestrator/skills.rs`
  (sitio en `pendientes`).
- **Esquema:** migración nueva con dos tablas (`notifications`, `ingest_keys`).
- **Frontend:** la sección de claves de ingesta en Ajustes.
- **Spec:** deltas nuevos en `notifications` e `ingest-keys`; deltas en `push`, `oidc-auth`,
  `frontend`, `tools/registry` y `orchestrator/skill-router`.

## Non-goals

- **Sin página de bandeja en el frontend.** El push abre la app; una vista de bandeja es otro change.
- **Sin notificaciones salientes.** Esto es una puerta de entrada, no una salida.
- **Sin rate limiting.** Queda como riesgo residual declarado.
- **Sin métricas ni Prometheus.** El proyecto no las tiene y este change no las estrena.
- **No se toca `docker-compose.prod.yml`.**
- **Sin `POST /api/push/test`.** No se añade un endpoint de prueba de push.
- **Sin cambiar la semántica de entrega del worker de recordatorios.** Allí solo se mueve el ayudante
  de host; el comportamiento es idéntico.
- **Sin listar las ya leídas.** El histórico es otra cosa.
