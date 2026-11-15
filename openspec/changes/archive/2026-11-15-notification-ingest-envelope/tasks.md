# Tasks

Todas las casillas empiezan sin marcar. **No se escribe ni una línea de código ni un solo test antes de que el usuario apruebe este change.**

Este change está **sin aprobar**: ninguna casilla está marcada, ni siquiera la 0.1, porque el protocolo OpenSpec exige la aprobación explícita del usuario antes de tocar código.

## 0. Aprobación (punto de parada del protocolo)

- [x] 0.1 Presentar este change (proposal + design + tasks + delta de `notifications`) y **esperar aprobación explícita** antes de tocar código. Ninguna casilla de las fases 1 a 4 se marca hasta esa aprobación.

## 1. Derivar `message` de `title` (contrato de la ingesta)

Fichero: `src/routes/notifications.rs` (`IngestRequest`, `validate`). Campos nuevos opcionales: `title`, `body_markdown`, `severity`.

- [x] 1.1 RED — tests de integración en `tests/api/notifications.rs` (patrón `app_with_key`, `oneshot`): (a) cuerpo sin `message` y con `title` no vacío responde `201` y persiste `message = title` recortado; (b) cuerpo sin `message` ni `title` (o ambos en blanco) responde `400`; (c) cuerpo con `message` y `title` distintos persiste el `message` (precedencia). Comando en rojo: `cargo test --test notifications`. **Pegar la salida en rojo.**
- [x] 1.2 GREEN — añadir `title: Option<String>` a `IngestRequest` y en `validate` derivar el mensaje: `message` si trae texto tras `trim`, si no `title`; si ninguno, `AppError::BadRequest("message is required")`. Confirmar `cargo test --test notifications` y `cargo check` en verde. Asegurar que `ingest_missing_or_blank_message_returns_400` sigue en verde (su cuerpo no trae `title`).

## 2. `severity` como fuente del nivel

- [x] 2.1 RED — tests: cuerpo sin `level` con `severity = "critical"` → `201` con `level = "critical"`; `severity = "WARNING"` (mayúsculas) → `warning`; `severity = "emergency"` → `201` con `level = "info"`; cuerpo con `level = "warning"` y `severity = "critical"` → persiste `warning`; `level = "debug"` (fuera de lista blanca) → sigue `400`. Comando: `cargo test --test notifications` (rojo).
- [x] 2.2 GREEN — añadir `severity: Option<String>` a `IngestRequest`; en `validate`, si falta `level`, mapear `severity` normalizado a minúsculas (`critical`/`warning`/`info`) y caer a `info` en cualquier otro caso; conservar la lista blanca y el `400` para un `level` presente fuera de ella. Verificar en verde.

## 3. `body_markdown` a `metadata`

- [x] 3.1 RED — tests: cuerpo con `body_markdown` no vacío → `metadata` persistida incluye `body_markdown` con ese valor; una `metadata` que ya traía `body_markdown` no se pisa; `body_markdown` vacío o solo espacios no añade la clave. Comando: `cargo test --test notifications` (rojo).
- [x] 3.2 GREEN — añadir `body_markdown: Option<String>` a `IngestRequest`; en `validate`, si trae texto tras `trim`, insertarlo en el objeto `metadata` bajo `body_markdown` solo si la clave no existe; validar el límite de 8192 bytes tras la fusión. Verificar en verde.

## 4. Documentación y cierre

- [x] 4.1 Actualizar `docs/api-notifications.md`: documentar `title` (fallback de `message`), `severity` (fallback de `level`, tolerante) y `body_markdown` (a `metadata`), con el ejemplo de envelope de OpenObserve.
- [x] 4.2 `just fmt`, `just clippy`, `just test` en verde; `openspec validate --all` en verde.
- [x] 4.3 PR a `development` con CI verde; `openspec archive notification-ingest-envelope`; renombrar `plans/PLAN-031.md` a `-ARCHIVED` y cerrar el issue #374.
