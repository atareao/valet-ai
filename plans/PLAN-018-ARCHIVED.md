# PLAN-018 — La elección de cierre reconoce cómo escribe el modelo de verdad

**Change OpenSpec asociado:** `prose-choice-widget`.
**Estado:** 🚧 En curso (abierto el 2026-11-06).

---

## 1. Motivo

El change `binary-choice-widget` (v0.28.0, archivado) añadió la proyección de una pregunta cerrada de sí/no a un
`QuickForm` con `Sí`/`No`. **No ha disparado ni una sola vez en producción.** El usuario lo detectó al probar la
app desplegada: preguntó algo y el agente respondió con una pregunta de dos caminos, sin widget.

## 2. Diagnóstico (evidencia medida sobre la BD real de `co1`)

Medición sobre los últimos 200 turnos del asistente:

| Comprobación | Resultado |
|:---|:---|
| Widgets con `Sí`/`No` en toda la BD | **0** |
| Turnos que disparan la regla actual (`ends_with('?')`) | 8 / 200 (4 %) |
| Turnos que dispararían ignorando el emoji final | 17 / 200 (8,5 %) |
| Turnos que cierran con pregunta (techo real) | 66 / 200 (33 %) |
| Turnos con pregunta de dos caminos «¿X o Y?» | 33 / 200 (16,5 %) |
| Ocurrencias de «o no?» en toda la BD | **0** |

Dos bloqueadores independientes, ambos por el estilo real del modelo:

1. **El modelo firma con emoji después del `?`.** Cierres reales: `…o va directo a algo concreto? ☕️`,
   `…? 😒☕️`, `…? 🎩`. `closed_yes_no_question` exige `question.ends_with('?')` y falla siempre.
2. **El modelo no pregunta «¿X o no?», pregunta «¿X o Y?».** `coordinates_named_alternatives` rechaza justo eso
   por diseño (D2 del plan anterior). Y en ese caso el widget correcto no es `Sí`/`No`, sino **los dos caminos
   como opciones**.

**Causa de fondo:** los tests del change anterior usaban texto sintético que acababa exactamente en `?` y decía
«o no». Pasaron en verde sin cubrir una sola respuesta real del modelo. La lección se incorpora al change como
corpus de regresión obligatorio.

## 3. Objetivo y alcance

**Objetivo:** que una pregunta de cierre produzca un widget de elección con las opciones que el modelo está
ofreciendo de verdad —`Sí`/`No` cuando la pregunta es binaria, los dos caminos cuando nombra dos alternativas—
sin tocar el frontend.

**Alcance:** `src/orchestrator/widget_projection.rs`, incluido su bloque `#[cfg(test)]`.

**Fuera de alcance:** el frontend (el contrato `QuickForm` ya sirve), la guía de widgets (`SKILL_WIDGETS_PROMPT`),
el esquema de base de datos y las demás formas reconocidas.

## 4. Diseño

- **F1 — Gate tolerante a la firma.** Antes del gate `ends_with('?')`, recortar los caracteres finales que no
  sean alfanuméricos ni `?`/`¿` (emoji, puntuación, cierre de comillas, marcadores Markdown). Orden:
  `strip_line_prefix` → `strip_markdown_emphasis` → recorte final → gate.
- **F2 — Forma «¿X o Y?».** Si la pregunta contiene una conjunción `o`/`ó`/`u` a nivel de palabra y la palabra
  siguiente **no** es `no`/`sí`/`si`, se parte por la **última** conjunción y se emiten **dos opciones** con los
  dos tramos. Si la palabra siguiente **sí** es `no`/`sí`/`si`, se mantiene `Sí`/`No`. Guardas: se rechaza si
  hay más de una conjunción (tres caminos), si algún tramo queda vacío o con menos de 3 caracteres
  alfanuméricos, o si algún tramo supera los 160 caracteres.
- **F3 — Última línea *con* pregunta del párrafo final.** Se busca hacia atrás la última línea con contenido que
  contenga `?`, sin cruzar una línea en blanco y sin salir de un bloque de código.
- **Modelo de datos:** `Presentation::BinaryChoice { question, span }` pasa a
  `Presentation::ClosingChoice { question, options: Vec<String>, span }`, con `options = ["Sí","No"]` o los dos
  caminos. El payload sigue siendo `QuickForm` con `fields[0].type = "radio"` y sin `title`.
- **Precedencia:** sin cambios. Si el turno ya produjo opciones, no se proyecta elección; si el modelo renderizó
  un `QuickForm`, gana el suyo; los `span` no se solapan.

## 5. Plan de ejecución

1. **SDD:** change `prose-choice-widget`, su delta de spec y este plan (aprobación antes de escribir código).
2. **TDD-RED:** corpus de regresión con las cadenas literales de producción, positivas y negativas, antes de
   tocar la implementación.
3. **TDD-GREEN:** F1, F2 y F3 en `widget_projection.rs`.
4. **TDD-REFACTOR:** limpieza, comentarios del porqué y `clippy` con cero warnings.
5. **Verificación:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --lib`,
   `just check-all`, `openspec validate --all --strict`.
6. **Auditoría:** `@rust-reviewer` sobre el diff.
7. **Release:** `release/vX.Y.Z` desde `development`, tag y despliegue en `co1`.
8. **Medición post-despliegue:** repetir el recuento sobre la BD real y confirmar que el número de widgets de
   elección deja de ser 0.

## 6. Criterios de aceptación

- [x] `> ¿Le doy el parte del día — tiempo, agenda y entrenamiento — o va directo a algo concreto? ☕️` produce
  un widget con los dos caminos.
- [x] `> ¿Cerramos por hoy, Lorenzo?` produce `Sí`/`No`.
- [x] `> ¿Hacemos esto o no?` sigue produciendo `Sí`/`No`.
- [x] `> ¿Qué necesita? 😒`, `> ¿Cuál, Lorenzo?` y `> ¿Prefieres A, B o C?` no producen widget.
- [x] `> Dígame. ☕️` no produce widget.
- [x] La pregunta aparece **una sola vez** en pantalla: como etiqueta del grupo de opciones.
- [x] Ninguna forma reconocida antes cambia de comportamiento.
- [x] Sin cambios en el frontend.
- [x] `just check-all` y `openspec validate --all --strict` en verde.

## 7. Riesgos

- **Falso positivo en prosa con `o`.** Mitigado por la guarda de una sola conjunción, el mínimo de 3 caracteres
  y el máximo de 160, más los negativos del corpus.
- **Opciones feas** (tramos largos con rayas o incisos). Mitigado por la limpieza de guiones y comas finales; se
  acepta que la etiqueta sea literal, porque inventar texto es peor.
- **Pregunta retórica del cuerpo.** Mitigado por F3: solo el párrafo final.
- **Regresión en las formas ya reconocidas.** Mitigado por la suite existente, que debe seguir en verde sin
  cambios de aserción.

## 8. Medición contra producción

Arnés desechable (`tests/zzz_corpus_measure.rs`, creado y borrado; no queda en el repo) sobre los últimos 200 turnos del asistente en la BD real de `co1`:

| Forma | Turnos | % |
|:---|---:|---:|
| Elección de cierre binaria (`Sí`/`No`) | 9 | 4,5 % |
| Elección de cierre de dos caminos | 32 | 16,0 % |
| **Total elección de cierre (lo nuevo)** | **41** | **20,5 %** |
| Opciones de tabla (ya existía) | 82 | 41,0 % |

Antes del change el contador era **0**. Ningún turno produjo más de un `QuickForm`.

Observaciones sin acción: en los turnos 67 y 156 el texto ofrece tres alternativas y el widget emite dos (fusiona las dos últimas en la segunda opción); en el turno 46 una tabla de opciones eclipsa una pregunta binaria del mismo turno, lo cual es la precedencia ya establecida (las opciones ganan a la elección de cierre) y no un defecto nuevo.
