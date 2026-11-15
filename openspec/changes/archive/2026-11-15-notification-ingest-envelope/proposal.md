# Proposal: Ingesta de notificaciones: aceptar webhooks genéricos (envelope de OpenObserve)

**Issue:** [#374](https://github.com/atareao/valet-ai/issues/374)

## Why

`POST /api/notifications` nació con un contrato pensado para productores propios: exige `message` y
un `level` de lista blanca. Al conectar la primera alerta de **OpenObserve** (la alerta
`traefik_high_403_429` en `co1`) la entrega llegó, pero Valet la rechazó con
`400 {"error":"message is required"}`: el webhook de una alerta **programada** de OpenObserve es un
envelope fijo que trae `title`, `body_markdown` y `severity`, y nunca `message`. No hay forma de que
OpenObserve emita el esquema de Valet (el modo `legacy` de sus destinos solo aplica a notificaciones
on-call, no a alertas programadas). Sin este change, Valet no puede recibir webhooks de una
herramienta de observabilidad real y la bandeja queda sin su caso de uso más natural: alertas
automáticas.

## What Changes

- **`message` SHALL ser opcional** en la ingesta: cuando falte, el mensaje SHALL derivarse de `title`
  (no vacío tras `trim`). Si ni `message` ni `title` aportan texto, la respuesta SHALL seguir siendo
  `400`. El contrato actual (cuerpo con `message`) permanece intacto.
- **SHALL aceptarse un campo opcional `severity`** que, en ausencia de `level`, SHALL determinar el
  nivel normalizando en minúsculas y mapeando `critical`/`warning`/`info` a la lista blanca existente;
  cualquier otro valor SHALL caer a `info` sin rechazar la petición.
- **SHALL aceptarse un campo opcional `body_markdown`** que, si viene no vacío, SHALL anexarse a la
  `metadata` persistida bajo la clave `body_markdown`, sin pisar una clave homónima ya presente, y
  respetando el límite de `metadata` existente.
- **El resto del envelope de OpenObserve SHALL ignorarse** (`version`, `fields`, `rows`, `links`,
  `context`, `chart_url`); el struct ya tolera campos desconocidos.

## Capabilities

### New Capabilities

- Ninguna.

### Modified Capabilities

- `notifications` (**MODIFIED**): el requirement «El cuerpo de la ingesta SHALL validarse y
  normalizarse antes de persistir» pasa a admitir `message` derivado de `title`, `severity` como
  fuente del nivel y `body_markdown` anexado a `metadata`; su escenario «Un mensaje ausente o en
  blanco se rechaza» se reescribe para exigir un `400` solo cuando **ni** `message` **ni** `title`
  aportan texto.

## Impact

- **Backend:** `src/routes/notifications.rs` (`IngestRequest` con `title`/`body_markdown`/`severity`
  opcionales y `validate` con la derivación del mensaje, el mapeo de `severity` y la fusión de
  `body_markdown` en `metadata`).
- **Tests:** `tests/api/notifications.rs` (escenarios nuevos y ajuste del de mensaje ausente).
- **Docs:** `docs/api-notifications.md` (documentar los campos del envelope).
- **Sin esquema:** no hay migración.
- **Sin frontend**, **sin autenticación**, **sin push**.

## Non-goals

- **No se cambia la autenticación** (sigue exigiendo token de ingesta) ni los endpoints.
- **No se interpreta el resto del envelope** (`fields`, `rows`, `links`, `context`, `chart_url`).
- **No se añaden severidades** fuera de la lista blanca (`error`, `debug`, ...): caen a `info`.
- **No se toca el push WebPush** ni el contenido del aviso.
- **No se modifica `message` cuando viene**: su `trim` y su límite de 4096 bytes no cambian.
