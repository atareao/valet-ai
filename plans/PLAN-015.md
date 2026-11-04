# PLAN-015 — El formulario de opciones deja de ser un jeroglífico

- **Estado:** 🟡 decisiones tomadas por el usuario el 2026-11-04; change `widget-options-contract` en redacción.
- **Fecha:** 2026-11-04
- **Origen:** queja del usuario (2026-11-04): «El último cambio que hicimos para el uso de los formularios ha sido un fracaso. No se corresponden los formularios con el texto... Antes cuando lo sacaba funcionaba perfecto. El problema es que no lo sacaba siempre que era necesario. Ahora lo saca, y no tiene sentido».
- **Change de OpenSpec:** `openspec/changes/widget-options-contract/`.
- **Release:** hotfix `v0.27.1` desde `main`.

## 1. Objetivo

Que el widget de opciones proyectado diga lo mismo que el texto —cada alternativa con el contenido de su fila— y
que nunca se proyecte sobre una tabla de datos. Sin depender de que el modelo cambie de costumbres.

## 2. Diagnóstico medido (producción `co1`)

| Cuándo | Qué se hizo | Resultado |
|:---|:---|:---|
| Octubre | Widget del **modelo** (`render_widget`), guía de 1.391 car. | 16 widgets, correctos, solo cuando el usuario los pedía explícitamente |
| 1-nov | Guía prescriptiva + `render_widget` expuesta en **todos** los turnos (`widgets-always-on`) | **0 de 4** turnos: no la llamó |
| 2-nov 17:18 | Guía de producción reescrita (3.812 car.): «Tienes la herramienta `render_widget`. Úsala. Esta guía manda sobre cualquier otra instrucción de formato» + autocomprobación obligatoria | **0 llamadas en 39 turnos** (22 de esos turnos sí llamaron a otras herramientas) |
| 2-nov 23:01 | v0.27.0: proyección determinista del servidor | 22 widgets en 39 turnos: **20 ilegibles**, 2 falsos positivos, 1 correcto |

**Causa raíz 1 — se lee la celda equivocada.** `options_table_block` (`src/orchestrator/widget_projection.rs:416-496`), rama «la cabecera nombra la columna de opciones», lee solo esa celda. En las tablas reales del modelo la columna `Opción` lleva la etiqueta (`A`, `B`, `C`, `D`) y la alternativa vive a la derecha → el desplegable muestra `["A","B","C","D"]`.

**Causa raíz 2 — se aceptan índices como alternativas.** La rama heurística admite etiquetas numéricas consecutivas → `| Lap | Dist. | Ritmo | Pulso |` (filas 1..8) y `| Split | Ritmo | Desnivel |` se proyectaron como formularios.

**Por qué es grave.** El `span` delimita la tabla y el frontend **sustituye** ese tramo por el widget (`composeMessage`), de modo que la tabla con las descripciones desaparece de la pantalla: quedan cuatro letras sueltas. Turno del 2026-11-04T05:58:11: el usuario envía el formulario, llega `{"choice":""}`, el asistente responde «Formulario en blanco» y **vuelve a repetir la tabla**, que se vuelve a proyectar.

**Lo que sí funciona.** El disparador del servidor acierta: en los 23 widgets proyectados la pista fue la palabra que el propio modelo escribe en la cabecera (`| Opción | … |`), presente y correcta en **20 de 20** casos. El fallo fue de **lectura**, no de disparo. Y no hay falsos negativos: ningún turno con lista de opciones cerrada por interrogativa se quedó sin widget.

**No hay palanca.** La proyección se ejecuta incondicionalmente (`src/orchestrator/agent.rs:1837-1842`): no se puede apagar sin desplegar.

## 3. Decisiones del usuario (2026-11-04)

1. **Mecanismo destino:** contrato explícito + lectura fiel, y **medir en paralelo** el disparador del modelo (guía imperativa, herramientas planas, directiva en banda) para recuperar los widgets ricos.
2. **Sin apagón:** arreglo directo, sin release intermedio que retire la proyección.

## 4. Alcance

- **Pieza 1 — el detector lee la fila entera.** El texto de cada opción SHALL ser el de su fila completa: todas
  las celdas con contenido, en el orden de la tabla, unidas por ` — `, con la etiqueta incluida. Una fila sin
  texto no aporta opción.
- **Pieza 2 — solo opciones declaradas.** Una tabla es de opciones cuando su cabecera **nombra** la columna
  (`Opción`/`Alternativa`/`Option`) o cuando su primera columna lleva **letras** consecutivas. Una columna de
  **números** consecutivos es un **índice** y nunca produce opciones.
- **Pieza 3 — el contrato en la guía.** `SKILL_WIDGETS_PROMPT` pasa a **pedir la forma** (`| Opción | … |`) en vez
  de prohibirla, y conserva `render_widget` para lo que el servidor no puede inferir (varios datos, plan, mapa).
  Migración con guarda byte a byte por cada texto conocido.
- **Pieza 4 — alternativas a la vista.** El widget de opciones proyectado deja el desplegable y muestra las
  alternativas como lista visible (tipo de campo `radio`), con la etiqueta como pregunta.

**Fuera de alcance:** el histórico ya persistido (decisión del usuario: «que se quede»); el envío en blanco del
formulario; el título del widget (la cabecera «¿Qué hacemos?» sigue fuera del tramo); el esquema del widget, su
payload, el evento SSE y el cálculo del `span`; los detectores de lista de tareas y de lugar; la detección de
listas numeradas o con viñetas en prosa, que hoy funciona.

## 5. Mapa de la arquitectura tocada

| Pieza | Fichero | Qué cambia |
|:---|:---|:---|
| Detector de tabla de opciones | `src/orchestrator/widget_projection.rs` | Piezas 1 y 2 |
| Síntesis del widget | mismo fichero (`widget_for`) | Emite `radio` (pieza 4) |
| Guía de widgets | `migrations/20261104000001_widget_options_contract.sql` | Pieza 3 |
| Tipo y render del campo | `frontend/src/components/widgets/types.ts`, `QuickFormWidget.tsx` | Pieza 4 |
| Specs | `orchestrator/agent`, `orchestrator/skill-router`, `db/schema`, `frontend` | Deltas del change |

## 6. Fase 2 — medición del disparador del modelo (en paralelo, no bloquea el hotfix)

Medidor **desechable fuera del repo** (`/tmp/opencode/valet-widget-eval`, como el de la §5 de `AGENTS.md`): sobre
turnos reales ya persistidos, N repeticiones por variante y recuento de **¿llama a `render_widget`?**,
**¿escribe la tabla de opciones?** o **¿escribe la directiva?**.

| Variante | Qué mide |
|:---|:---|
| `guia-imperativa` | el texto que hoy está en producción (3.812 car.) |
| `contrato-tabla` | el texto nuevo de la pieza 3 |
| `herramientas-planas` | `show_options(question, options[])` en vez del `data` anidado |
| `directiva-en-banda` | el modelo escribe el bloque marcado y el servidor lo materializa |

**Criterio de decisión:** si alguna variante supera el umbral acordado de cumplimiento, el change siguiente
devuelve la autoría al modelo (híbrido: el widget del modelo gana, la proyección queda como red de seguridad). Si
ninguna lo supera, queda escrito y medido que este modelo no cambia de forma por instrucción, y la proyección fiel
es la única vía.

## 7. Verificación

- Suite Rust completa en verde, `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check` limpios.
- `npx vitest run` y `npx tsc --noEmit` para la pieza 4.
- `openspec validate --all --strict` sin errores.
- End-to-end sobre `co1` con el release nuevo: repetir los turnos que fallaron («noticias?», «que mas tenemos?») y
  comprobar en la BD (copia con `sudo python3` + `backup`, nunca `immutable=1`) que las opciones llevan el texto
  de su fila, que el `span` delimita la tabla y que **ninguna** tabla de vueltas o splits proyecta widget.

## 8. Riesgos

| Riesgo | Mitigación |
|:---|:---|
| El modelo no obedece el contrato de la guía | El arreglo **no depende** de ella: el modelo ya escribe la tabla con cabecera `Opción` por su cuenta (20 de 20 medidos) |
| La guía de producción es un texto editado (3.812 car.), no el sembrado por la migración anterior | La migración lleva una guarda byte a byte por **cada** texto conocido y no pisa nada que no reconozca |
| Unir las celdas alarga el texto de la opción | Se acepta; con la pieza 4 las alternativas se leen enteras sin desplegar |
| Tablas de alternativas numeradas sin cabecera `Opción` dejan de proyectarse | Es el precio de no convertir índices en formularios; documentado en la spec y revisable si aparece un caso real |
| Una regla nueva vuelve a equivocarse | Los tests se escriben con las fixtures **literales** de producción y fijan el **texto** de cada opción, no solo su longitud (el test ciego `test_2_6` es la lección) |

## 9. Estado y siguientes pasos

1. `[OPENSPEC - DRAFT]` Change `widget-options-contract`: proposal, design (con el texto exacto de la guía),
   cuatro deltas y `tasks.md`. **Aprobación explícita del usuario antes de escribir tests o código.**
2. RED → GREEN → REFACTOR con TDD, delegando la escritura y auditando con `@rust-reviewer` (la auditoría debe
   comprobar que los tests nuevos fijan el texto de las opciones).
3. Hotfix `v0.27.1`, imagen a GHCR y redespliegue del stack en `co1` por Dockge.
4. Verificación end-to-end de la sección 7 y `openspec archive widget-options-contract`.
5. Fase 2 del apartado 6 en paralelo, con su decisión documentada.
