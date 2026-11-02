# Tasks — widget-projection

## 1. Planificación (SDD)

- [x] 1.1 Crear el change `openspec/changes/widget-projection/`.
- [x] 1.2 Escribir `proposal.md` (why / what changes / impact) apoyado en la evidencia numérica del
      2-nov-2026 (guía viva y 0/3; aislamiento 0/3, 1/3, 0/3; encuesta de industria).
- [x] 1.3 Escribir `design.md` (contexto, D1–D8, riesgos, tabla de alternativas descartadas, plan de
      migración y preguntas abiertas).
- [x] 1.4 Escribir los deltas: `specs/orchestrator/agent/spec.md` (2 requisitos añadidos),
      `specs/orchestrator/skill-router/spec.md` (el requisito de la guía modelado como
      `REMOVED` + `ADDED` —`MODIFIED` no puede perder escenarios y dos de los heredados describen la
      guía prescriptiva derogada—, con el encabezado viejo en `REMOVED` y uno nuevo en `ADDED`, porque
      el CLI rechaza el mismo encabezado en ambos bloques),
      `specs/db/schema/spec.md` (1 requisito modificado, para actualizar la referencia cruzada al
      encabezado nuevo), `specs/stream-route/spec.md` (1 requisito modificado) y
      `specs/frontend/spec.md` (1 requisito añadido).
- [x] 1.5 Validar con `openspec validate widget-projection --strict`.
- [x] 1.6 **Decisión de almacenamiento (D2)**: el usuario aprueba el 2026-11-02 la opción **(c)** —el contenido
      se persiste íntegro y el widget declara el tramo que ocupa (`span`)— en lugar de recortar el texto. Los
      artefactos se actualizan a esa decisión y se revalidan (`openspec validate --all --strict`: 63 passed,
      0 failed; `just check-spec`: verde).
- [x] 1.7 **Aprobación explícita del usuario**: otorgada el 2026-11-02. El change queda aprobado y el TDD
      arranca en una sesión nueva (el relevo está en `plans/PLAN-013.md` → §10).

## 2. RED — el proyector puro: cada detección

- [x] 2.1 Crear `src/orchestrator/widget_projection.rs` con las firmas públicas (`detect`, `synthesize`, los
      tipos `Presentation` y `Widget`, y el tipo del tramo) y **sin** implementación, de modo que el módulo
      compile con los tests en rojo. `Presentation` SHALL llevar el tramo del bloque reconocido y `Widget`
      SHALL llevar el `span` en su payload.
- [x] 2.2 Tests de `detect` para `Options`: opciones etiquetadas `A)`/`B)`, `A.`/`B.`, `A:`/`B:`, `1)`/`2)`,
      `1.`/`2.` y `Opción A`/`Opción B`; y alternativas en viñetas cerradas por una interrogativa del tipo
      «¿cuál…?». Cada test SHALL comprobar además que el tramo delimita **exactamente** el bloque.
      Verificación: `cargo test widget_projection` — cada test falla por el `todo!`/`None` inicial.
- [x] 2.3 Tests de `detect` para `Checklist`: tabla Markdown cuya primera columna es un índice (`| # | Tarea |`),
      tabla con casilla (`| [ ] | … |`), y lista de viñetas con forma de tarea. Igual: comprobar el tramo.
- [x] 2.4 Tests de `detect` para `Location` **solo con coordenadas** (D1, decidida en el RED: la forma
      «dirección» queda aplazada). Se cubren el par etiquetado y el par con decimales; una dirección
      suelta se cubre como **NO detección**.
- [x] 2.5 Test del contrato del tramo: es un rango **semiabierto** de **bytes UTF-8** —`content[start..end]`
      corta sin pánico y devuelve el bloque—, la frase de encabezado y la de cierre quedan fuera, y dos bloques
      del mismo texto no producen tramos solapados.
- [x] 2.6 Ejecutar `cargo test widget_projection` y confirmar que **solo** fallan los tests nuevos.

## 3. RED — el proyector puro: cada NO detección (el riesgo real)

- [x] 3.1 Tests de NO detección: un dato único («Son las 18:30»), una explicación sin listas, una
      tabla Markdown sin columna índice/casilla, un número suelto con decimales (que **no** es una
      coordenada), una enumeración dentro de una frase («puedes elegir entre A y B, pero…» sin lista) y
      una lista numerada que es un **procedimiento** y no opciones («1. Abre el fichero; 2. Edita…»).
- [x] 3.2 Test de que un texto vacío o solo espacios no detecta ningún bloque (`Vec` vacío).
- [x] 3.3 Ejecutar `cargo test widget_projection` y confirmar el rojo de los casos nuevos.
- [ ] 3.4 **Criterio de aceptación del RED**: cada NO detección es un test que falla por la
      implementación ausente, no por un test mal escrito. Revisar que ninguno pase por accidente.

## 4. GREEN — el proyector puro

- [x] 4.1 Implementar `detect` con los parsers mínimos que hacen pasar 2.2–2.6 y 3.1–3.2. **Ante la duda,
      `None`**: el falso positivo es peor que el falso negativo (D3).
- [x] 4.2 Implementar `synthesize`: `Options` → `QuickForm` con un campo `select`; `Checklist` → `Checklist`
      con `items: [{id, label}]`; `Location` → `LocationWidget` con `latitude`/`longitude` (`address`
      opcional). Generar un `id` único por widget y **propagar el tramo** del bloque reconocido. Preferencia de
      datos: `data` de la herramienta del turno si la produjo; si no, el texto (D3).
- [x] 4.3 Verificar el contrato del tramo (2.5) en verde: delimita el bloque, respeta los límites de carácter de
      UTF-8 y no se solapa con el de otro widget del mismo mensaje.
- [x] 4.4 Tests de `synthesize` con datos de herramienta y sin ella, y de que el widget resultante lleva el
      tramo del bloque del que salió.
- [x] 4.5 `cargo test widget_projection` en verde y `cargo check` sin errores.

## 5. RED — la integración en el turno

- [x] 5.1 Test end-to-end en `src/orchestrator/agent.rs`: un turno cuyo texto final presenta una lista
      de opciones y que **no** llama a `render_widget` acaba con `messages.widgets` no vacío y con un
      evento SSE `widget` **antes** del `done`.
- [x] 5.2 Test de fusión: un turno que ya invocó `render_widget` (Checklist) y cuyo texto presenta una
      lista de tareas acaba con **un solo** widget de ese tipo (el del modelo) — la proyección se omite
      para ese tipo.
- [x] 5.3 Test de que un turno sin forma reconocible deja `widgets` como `null` y no emite `widget`.
- [x] 5.4 Test del contenido íntegro y del tramo: el `content_buffer` persistido **contiene** el bloque
      proyectado tal y como lo escribió el modelo; el widget persistido lleva un `span` que delimita
      exactamente ese bloque; y el evento `done` lleva un campo `content` igual al contenido persistido.
- [x] 5.5 Test de serialización: el `done` con el campo `content` (extiende `test_done_serialization`) y el
      evento `widget` con el campo `span` **opcional** (ausente cuando el widget viene del modelo).
- [x] 5.6 Ejecutar `cargo test` y confirmar el rojo de los tests nuevos (el resto en verde).

## 6. GREEN — la integración en el turno

- [x] 6.1 En `src/orchestrator/agent.rs`, en el cierre del turno y **antes** de
      `MessagesRepo::set_widgets` / `publish_persisted_message`: `detect` sobre `content_buffer` →
      `synthesize` → fusionar con `rendered_widgets` deduplicando por nombre + payload (el del modelo gana)
      → persistir el texto **íntegro, sin recortar** → `set_widgets` con la lista fusionada, con el `span` de
      cada widget proyectado.
- [x] 6.2 Emitir el/los evento(s) SSE `widget` de la proyección **antes** del evento `done`, con el `span` del
      bloque que cada uno ocupa. El `id` de cada widget proyectado SHALL generarse una sola vez y reutilizarse
      para el evento y para la fila persistida (misma identidad `[widget:Name#id]`).
- [x] 6.3 Añadir el campo `content` a `SSEEvent::Done` (el texto íntegro persistido) y el campo **opcional**
      `span` a `SSEEvent::Widget` (`#[serde(skip_serializing_if = "Option::is_none")]`), sin cambiar la
      serialización de los eventos de widgets del modelo.
- [x] 6.4 `cargo test` y `cargo check` sin errores.

## 7. La migración y el recordatorio

- [ ] 7.1 Escribir `migrations/20261102000001_widget_projection.sql`: sustituir
      `SKILL_WIDGETS_PROMPT` por el recordatorio breve **solo si** su valor es byte a byte el sembrado
      por `20261101000001_widgets_always_on.sql`.
- [ ] 7.2 Transcribir a la migración, **carácter a carácter**, el texto exacto del recordatorio aprobado
      por el usuario el 2026-11-02 (el literal del requisito de `orchestrator/skill-router`):

      > Los widgets los proyecta el servidor a partir de tu respuesta: si presentas una lista de
      > opciones, una lista de tareas o un lugar, aparecerá un widget. No lo anuncies ni repitas la
      > lista en prosa; escribe solo la prosa que lo acompaña. Usa `render_widget` únicamente si
      > necesitas un widget con datos más ricos de los que el servidor puede inferir.
- [ ] 7.3 Test de la migración: el fragmento prístino se sustituye; el editado se respeta; la
      migración es idempotente.
- [ ] 7.4 Test del catálogo: el recordatorio **no** enumera los tres widgets con ejemplos de `data` ni
      enuncia la regla de las opciones como instrucción del modelo, y remite a `render_widget` como vía
      explícita.
- [ ] 7.5 `cargo test migrations` y `cargo test skills` en verde.

## 8. Frontend — la composición por tramos

- [ ] 8.1 `frontend/src/components/widgets/types.ts`: añadir `span?: [number, number]` a `WidgetInstance`.
- [ ] 8.2 `frontend/src/hooks/useSSE.ts`: pasar el campo `content` del evento `done` al callback y el campo
      `span` del evento `widget` a `onWidget`.
- [ ] 8.3 `frontend/src/hooks/useMainChat.ts`: conservar el `span` en el widget asociado al mensaje en curso
      (también al reconstruir el historial desde `messages.widgets`) y, al recibir un `done` con `content`
      presente, fijar el contenido del mensaje en curso a ese valor autoritativo, sin perder los widgets ni su
      identidad.
- [ ] 8.4 `frontend/src/components/widgets/compose.ts`: **módulo puro** `composeMessage(content, widgets)` que
      devuelve los tramos ordenados (texto / widget). Con tests unitarios de los casos límite: tramo válido,
      widget sin `span`, `span` fuera de rango, `span` solapado con otro, contenido vacío y `span` que cae en
      medio de un carácter multibyte.
- [ ] 8.5 `frontend/src/components/MessageBubble.tsx`: pintar los tramos en orden en lugar del texto seguido de
      los widgets.
- [ ] 8.6 Test de componente: el widget se pinta en el sitio del tramo, la lista en prosa no se pinta, y un
      `span` inválido cae al render de hoy (texto íntegro + widgets debajo) sin error.
- [ ] 8.7 `npx vitest run` en verde y `npx tsc --noEmit` sin errores.

## 9. REFACTOR y suite completa

- [ ] 9.1 `cargo clippy --all-targets -- -D warnings` sin avisos.
- [ ] 9.2 `cargo fmt --check`.
- [ ] 9.3 Revisar que no queda código muerto: `detect`/`synthesize` se usan en el cierre del turno,
      `composeMessage` se usa en `MessageBubble`, y los tipos `Presentation`/`Widget` no tienen variantes ni
      campos sin uso.
- [ ] 9.4 `just check-all` verde.

## 10. Verificación end-to-end (sustituye a la batería 6.3 de `widgets-always-on`)

> La batería 6.3 medía la guía contra el modelo y falló. Esta sección la sustituye por verificaciones
> **deterministas**, que no dependen del modelo.

- [ ] 10.1 `openspec validate --all --strict` sin errores.
- [ ] 10.2 **Verificación del proyector (sin modelo)**: un test que, dado un texto de respuesta real con
      una lista de opciones (uno de los tres prompts que fallaron el 1-nov), el proyector produce el
      widget esperado con las opciones del texto. Es la versión determinista de la batería.
- [ ] 10.3 **Verificación del camino del agente (sin modelo)**: un turno simulado cuyo texto final
      presenta una lista acaba con `messages.widgets` no vacío y con un evento `widget` antes del
      `done` (cubierto por 5.1; se repite aquí como comprobación de conjunto).
- [ ] 10.4 **Verificación sobre el despliegue**: con el release desplegado, enviar un mensaje real que provoque
      una respuesta con lista de opciones y comprobar en la BD que `messages.widgets` deja de estar vacío, que
      el contenido persistido **conserva la lista íntegra** y que el widget persistido lleva un `span` que
      delimita ese bloque. En `co1` no hay `sqlite3`: usar `sudo python3` con
      `sqlite3.connect("file:...?mode=ro", uri=True)` más `backup` —la BD está en WAL; nunca `immutable=1`—.
      Volumen: `valet_valet_data` → `/var/lib/docker/volumes/valet_valet_data/_data/valet.db`.
- [ ] 10.5 **Verificación de la colocación en la UI**: con el release desplegado, comprobar que el chat muestra
      el widget **en el sitio** de la lista (no debajo) y que la lista en prosa no se ve.
- [x] 10.6 **Decisión sobre D4**: ejecutado el 2026-11-02 por aprobación del usuario (D4), **antes** de
      empezar el código: `openspec archive widgets-always-on --yes`, documentando que la batería 6.3
      quedó sustituida por 10.2–10.5. El change quedó en
      `openspec/changes/archive/2026-11-02-widgets-always-on`.
