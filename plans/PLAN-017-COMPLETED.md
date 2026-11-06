# PLAN-017 — La pregunta cerrada de sí/no ofrece un widget de elección

**Change OpenSpec asociado:** `binary-choice-widget`.
**Estado:** ✅ Completado (2026-11-06, v0.28.0).

---

## 1. Motivo

Al probar el agente desplegado, el usuario detectó un hueco: «funciona perfecto, salvo cuando son cuestiones
como "¿hacemos esto o no?", "¿te parece bien?"; en estos casos debería ofrecer sí o no». Son precisamente las
formas con las que el agente pide una decisión, y hoy no producen ningún widget: hay que escribir la respuesta
a mano cuando la interacción esperada es un toque.

## 2. Diagnóstico (evidencia)

`src/orchestrator/widget_projection.rs` reconoce tres formas —opciones, lista de tareas y lugar— y **ninguna**
cubre una pregunta de sí/no:

- `question_options_block` (línea 709) descarta la tanda si no tiene **dos o más** ítems escritos
  (`if end - index < 2 { return None }`, línea 714).
- `is_interrogative` (línea 696) solo acepta la familia «¿cuál…?»
  (`contains_word(&lower, "cuál") || contains_word(&lower, "cual")`), con el comentario explícito:
  «a plain yes/no question leaves a procedure as a procedure».

Consecuencia: «¿Hacemos esto o no?» no tiene lista y no lleva «cuál», así que `detect` devuelve vacío y
`widgets` queda `null`.

## 3. Objetivo y alcance

**Objetivo:** que una **pregunta cerrada de sí/no que cierre el turno** acabe en un widget de elección con `Sí`
y `No`, con la pregunta dentro del widget y sin tocar el frontend.

**Alcance:** `src/orchestrator/widget_projection.rs`, incluido su bloque `#[cfg(test)]`.

**Fuera de alcance:** el frontend (el contrato `QuickForm` ya sirve), la guía de widgets
(`SKILL_WIDGETS_PROMPT`), el esquema de base de datos y el resto de formas reconocidas.

## 4. Diseño

- **D1 — Disparador:** solo la **última línea con contenido** del turno.
- **D2 — Precisión:** sin interrogativo abierto (`qué`, `cuál`, `quién`, `cómo`, `cuándo`, `dónde`, `cuánto`,
  `por qué`) y sin alternativas nombradas coordinadas con `o` (salvo `o no`/`o sí`).
- **D3 — La pregunta viaja dentro del widget:** el `span` **sustituye** el texto (en
  `frontend/src/components/widgets/compose.ts` se recorta con `cursor = span.end`), así que la pregunta se
  emite como **etiqueta del grupo de opciones** y el widget **no** lleva `title` (si lo llevara, la etiqueta se
  derivaría de él y aparecería duplicada).
- **D4 — Presentación:** `QuickForm` con `fields[0].type = "radio"` y `options = ["Sí", "No"]`.
  `QuickFormField.label` ya es obligatorio en los tipos del frontend, así que **no hay trabajo de frontend**.
  Dos toques (elegir + *Enviar*); el widget binario de un toque queda para un change posterior.
- **D5 — Precedencia:** si el turno ya produjo opciones, la elección binaria no se proyecta; si el modelo
  renderizó un `QuickForm`, gana el suyo (regla de fusión existente).
- **D6 — Riesgo:** el falso positivo. Se acota con D1/D2 y se blinda con los escenarios negativos.

## 5. Plan de ejecución

1. **SDD:** change `binary-choice-widget`, su delta de spec y este plan (aprobación antes de escribir código).
2. **TDD-RED:** tests de la función pura, positivos y negativos.
3. **TDD-GREEN:** la forma en `Presentation`, su detección en `detect` y su síntesis en `widget_for`.
4. **TDD-REFACTOR:** limpieza, comentarios del porqué y `clippy` con cero warnings.
5. **Verificación:** `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test --lib`, `just check-all`,
   `openspec validate --all --strict`.
6. **Auditoría:** `@rust-reviewer` sobre el diff.
7. **Release:** `release/vX.Y.Z` desde `development`, tag y despliegue en `co1`.

## 6. Criterios de aceptación

- [x] Una pregunta de sí/no que cierra el turno produce un widget de elección con `Sí` y `No`.
- [x] La pregunta aparece **una sola vez** en pantalla: como etiqueta del grupo de opciones.
- [x] Una pregunta abierta, con alternativas nombradas, en medio del texto, dentro de un bloque de código o en
  un turno que ya proyectó opciones **no** produce elección binaria.
- [x] Ninguna forma reconocida antes cambia de comportamiento.
- [x] Sin cambios en el frontend.
- [x] `just check-all` y `openspec validate --all --strict` en verde.

## 7. Riesgos

- **Falso positivo**: mitigado por D1/D2 y por los escenarios negativos de la spec.
- **Pregunta duplicada o perdida**: mitigado por D3 y su escenario.
- **Mezcla con la forma de opciones**: mitigado por la regla de precedencia de D5.

## 8. Estado

🚧 En curso. Abierto el 2026-11-05 a partir de la observación del usuario al probar el agente desplegado.
