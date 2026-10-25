# Change: Guardas del bucle ReAct para llamadas a herramientas

## Why

El bucle ReAct limita hoy las llamadas a una herramienta con **una sola** guarda:
`MAX_TOOL_RETRIES = 5` (`src/orchestrator/agent.rs:256`), contada por la clave `tool::operation`
(o el nombre del tool si no hay `operation`). El contador suma **todas** las llamadas a esa clave
**con independencia de los argumentos y del resultado**, así que no distingue dos situaciones
opuestas:

- **Progreso:** marcar 10 correos distintos como leídos son 10 llamadas a `email::mark_read`, cada
  una con un `id` distinto y un resultado distinto. Trabajo legítimo.
- **Bucle:** el **mismo** correo reintentado 10 veces, normalmente porque falla.

Ambas caen en la misma clave y mueren en la 6ª. El síntoma real: al pedir «marca 10 correos como
leídos» el agente recibe el corte tras la quinta y no termina la tarea.

La guarda está además **desalineada consigo misma**: el mensaje fabricado dice *«has been called
3 times»* y el comentario de `agent.rs:1373` dice *«max 3»*, pero la constante vale `5`; y la spec
`orchestrator/agent` titula el requisito *«Per-tool max retry limit of 3»*. Tres fuentes, tres
números.

El objetivo del límite —romper bucles que no avanzan— es correcto; lo que falla es la **métrica**.
Subir el número a secas lo empeora: con la clave actual un bucle real (misma llamada fallando)
quemaría más intentos y, como `max_iterations = 10` corta antes, el contador por herramienta
dejaría de existir en la práctica: el modelo farfullaría diez rondas en vez de parar en tres.

## What Changes

- La guarda única SHALL sustituirse por **tres guardas ortogonales**:
  1. **Huella** — corta la repetición de la **misma** llamada (mismo tool + `operation` +
     argumentos normalizados), sea cual sea su resultado.
  2. **Presupuesto por `tool::operation`** — contabiliza por separado **éxitos** (techo alto) y
     **fallos consecutivos** (umbral bajo), de modo que marcar N elementos distintos nunca choque
     con el tope de éxitos y un fallo repetido se corte pronto.
  3. **Tope global del turno** — acota el total de llamadas ejecutadas en un turno.
- Los cuatro umbrales SHALL vivir en `settings` y SHALL leerse **en cada turno** (como los
  `GENERATION_*`), de modo que editarlos en la interfaz surta efecto sin reiniciar, con fallback al
  default compilado y `warn!` si el valor no es parseable.
- El mensaje que recibe el modelo al cortar SHALL ser **exacto** e indicar **qué** guarda saltó
  (huella / fallo / éxito / tope global), en vez del texto fijo y desfasado actual.
- `MAX_TOOL_RETRIES` SHALL desaparecer como constante; su papel lo cubren las tres guardas.
- Los cuatro umbrales SHALL poder editarse desde la interfaz (sección nueva en Ajustes).
- La spec `orchestrator/agent` SHALL corregirse: el requisito del límite (que dice «3») y el
  requisito de logging (que menciona «3») SHALL reflejar el nuevo contrato.

## Impact

- Código: `src/orchestrator/agent.rs` (bucle ReAct), nuevo módulo `src/orchestrator/tool_limits.rs`
  (lectura de umbrales, al estilo de `src/generation.rs`), `src/db/repos/settings.rs` (tests
  guardianes de sembrado), `migrations/2026…_react_tool_guards.sql`. Interfaz:
  `frontend/src/components/SettingsDialog.tsx` (+ tipos y hook de ajustes).
- Specs: `orchestrator/agent` (1 requisito eliminado, 1 añadido, 1 modificado), `db/repos`
  (1 requisito añadido).
- Claves de `settings` nuevas (no sensibles: la API de ajustes ya las expone automáticamente).
- Sin cambios en el contrato de las herramientas, sin migración de datos con pérdida, sin tocar
  `docker-compose.prod.yml`.

## Defaults propuestos

| Clave | Significado | Default |
|---|---|---|
| `REACT_TOOL_FINGERPRINT_LIMIT` | ejecuciones permitidas de una llamada idéntica | `2` |
| `REACT_TOOL_FAILURE_LIMIT` | fallos **consecutivos** por `tool::operation` antes de cortar | `3` |
| `REACT_TOOL_SUCCESS_LIMIT` | llamadas **exitosas** por `tool::operation` antes de cortar | `50` |
| `REACT_TOOL_TOTAL_LIMIT` | llamadas ejecutadas en total en el turno | `100` |

## Non-goals

- NO se hace configurable `max_iterations` (sigue en `10`, cota externa y ortogonal).
- NO se toca `docker-compose.prod.yml`.
- NO se cambia la implementación de las herramientas ni sus descripciones. Que `mark_read` acepte
  un array de `ids` (y marque todas si se omite) ya existe; empujar al modelo a usarlo es otro
  trabajo, no este change.
- NO se persisten los contadores: viven en memoria y se reinician en cada turno.
