# Design

## Context

Ver `proposal.md` → *Why*. Lo que condiciona el diseño:

- **`render_widget` no está en `CORE_TOOLS`** (`get_current_time` y `get_current_location`) y un
  requisito de `orchestrator/skill-router` lo prohíbe explícitamente: «La herramienta `render_widget`
  SHALL pertenecer a la skill `widgets` y SHALL NOT estar en el core». Ese requisito se respeta.
- **El umbral efectivo se resuelve en `effective_threshold`**: el override `ROUTER_THRESHOLD_<ID>`
  si existe, y si no el `spec.threshold` compilado. La selección es
  `probability >= effective_threshold`.
- **Un `answers.get(spec.id)` ausente hoy descarta la skill** (`let Some(...) else { continue; }`),
  de modo que un umbral cero no basta por sí solo para garantizar la selección.
- **El fallo abierto expone todas las herramientas habilitadas pero deja `selection.skills` vacío**
  (`SkillRouter::select` → closure `fallback`), y `compose_skill_fragments` filtra por
  `selection.skills`. Hoy, con el router caído o apagado, el modelo recibe herramientas **sin
  ninguna guía de skill**. Es una incoherencia preexistente que este cambio tiene que tapar, porque
  «siempre presente» incluye los turnos en que el router falla.
- **La interfaz ya sabe representar un umbral extremo**: `skill-router-ui` exige avisar, sin
  bloquear, cuando el umbral de una skill está en cero o en uno. Un umbral cero no es un estado
  nuevo para la UI.
- **La migración `20261008000001` sembró `SKILL_WIDGETS_PROMPT` con 1.391 caracteres.** El valor
  medido en producción es byte a byte ese texto: nadie lo ha editado.

## Goals / Non-Goals

**Goals:**

- Que `render_widget` figure en el conjunto expuesto y su guía en el mensaje de sistema **en todos
  los turnos**, sin depender de lo que opine el clasificador.
- Que la guía sea **suficiente por sí sola** para que el modelo renderice cuando toca, porque es la
  única palanca que queda una vez que la herramienta está siempre delante.
- Que el usuario conserve la palanca: poder volver a condicionar `widgets` subiéndole el umbral.

**Non-Goals:**

- No se mueve `render_widget` a `CORE_TOOLS`.
- No se introduce un concepto nuevo de skill «siempre activa» en el catálogo.
- No se toca el frontend, ni el esquema de la herramienta, ni la persistencia de widgets.
- No se implanta la detección determinista de listas de opciones en prosa (red de seguridad de otra
  fase).

## Decisions

### D1 — Umbral cero como «no condicionada», en vez de una bandera `always_on`

**Decisión**: la skill `widgets` pasa a umbral compilado `0`, y se añade una regla **general** al
router: una skill cuyo umbral efectivo sea cero no está condicionada por el clasificador y se
selecciona siempre.

**Por qué**:

- El umbral cero **ya es un estado que el sistema entiende y la UI sabe avisar**; una bandera nueva
  obligaría a tocar el catálogo, la API del catálogo, el tipo `SkillInfo` del frontend, la pestaña
  «Skills» y sus tests, para un resultado idéntico.
- El usuario conserva la palanca: subir el umbral vuelve a condicionar la skill sin desplegar.
- Es honesto: no deja en la UI un campo de umbral, una pregunta y unos criterios que no hacen nada.

**Alternativa descartada**: una bandera `always_on` en `SkillSpec` que sacara `widgets` del conjunto
enrutable. Funciona igual, pero obliga a que la pestaña de `widgets` deje de ofrecer umbral, pregunta
y criterios —campos que quedarían inertes— y a modificar el catálogo, la API, el tipo del frontend y
los tests de la UI. Más superficie para el mismo efecto.

### D2 — La ausencia de respuesta no desactiva una skill a umbral cero

**Decisión**: si el clasificador no devuelve entrada para una skill cuyo umbral efectivo es cero,
esa skill **se selecciona igualmente**.

**Por qué**: sin esta regla, «umbral cero» sería «casi siempre» y no «siempre». El clasificador
puede omitir una pregunta (respuesta parcial), y el turno perdería la herramienta por un fallo ajeno.
La ausencia **no** cuenta como respuesta reconocida: la regla de respuesta inútil del router se queda
como está.

### D3 — El fragmento de una skill a umbral cero viaja en todos los turnos, incluido el fallo abierto

**Decisión**: `compose_skill_fragments` considera **activa** una skill habilitada cuyo umbral efectivo
sea cero, al margen del `SelectionSource`.

**Por qué**: hoy el fallo abierto expone todas las herramientas pero no inyecta ninguna guía, porque
`selection.skills` va vacío. Con `render_widget` siempre expuesta, ese hueco dejaría al modelo con la
herramienta delante y sin instrucciones justo cuando el router falla. Una skill deshabilitada sigue
siendo un filtro absoluto y no inyecta nada.

### D4 — La guía se ancla en comportamiento observable, no en criterios abstractos

**Decisión**: la guía nueva se construye sobre una regla verificable por el propio modelo en el
momento de escribir —«si vas a escribir dos o más opciones etiquetadas (A/B/C, 1/2/3) o una lista de
alternativas, NO las escribas: renderiza un widget»—, nombra los tres widgets con un ejemplo mínimo
de `data` para cada uno, y **retira la puerta de salida** de la guía actual.

**Por qué**: los tres defectos medidos de la guía vigente son exactamente esos: (1) no nombra ningún
widget, así que el modelo tiene que deducir el enum y construir un `data` cuyo esquema solo vive en
600 caracteres de JSON Schema; (2) sus disparadores son abstractos («el usuario deba elegir entre 2 o
más opciones») y compiten con un prompt base que pide brevedad; (3) su regla 3 autoriza expresamente
la prosa cuando el modelo considera que «la respuesta se resuelve con una explicación», que es la
salida que los datos de producción muestran usándose. La regla nueva es la única que se puede
comprobar desde dentro del propio turno: no depende de clasificar la intención, sino de lo que el
modelo está a punto de escribir.

**Se conserva** la regla de honestidad (un widget solo aparece si se ejecuta la herramienta) y la de
procesamiento de la respuesta del widget.

### D5 — La migración solo pisa lo que siga siendo el sembrado

**Decisión**: la migración elimina `ROUTER_THRESHOLD_WIDGETS` **solo si vale `0.20`** (el sembrado) y
sustituye `SKILL_WIDGETS_PROMPT` **solo si su valor es byte a byte el texto de la migración
`20261008000001`**. El umbral compilado pasa a `0`, de modo que la ausencia de la clave también da
cero.

**Por qué**: es el idioma que ya usan las migraciones del enrutador (solo pisan el valor sembrado) y
respeta a quien haya personalizado la guía o subido el umbral a propósito. Eliminar la clave en vez
de escribirla deja el valor efectivo en el compilado y evita marcar un override que el usuario no ha
hecho.

### D6 — Sin cambios en el frontend

**Decisión**: ni una línea de frontend.

**Por qué**: el umbral de `widgets` aparecerá en `0` en su pestaña de «Skills», y la interfaz ya
avisa —sin bloquear— cuando un umbral está en los extremos. Ese aviso pasa a ser **permanente** en la
pestaña de `widgets`, y es informativo: dice la verdad, que la skill no está condicionada. Convertirlo
en una nota específica es una mejora de UI que puede hacerse después, sin tocar este cambio.

## Risks / Trade-offs

- **La guía es un cambio de prompt y no tiene test unitario.** Los tests cubren el *mecanismo*
  (selección, exposición, inyección), no que el modelo renderice. Mitigación: la batería medida de la
  tarea 6.3 sobre el despliegue, con los tres casos reales que fallaron hoy.
- **Coste por turno**: el esquema de `render_widget` viaja siempre. Se acepta a cambio de que la
  herramienta esté siempre presente.
- **Widgets de más**: con la herramienta siempre delante, el riesgo contrario —renderizar cuando no
  toca— lo acota la sección «Cuándo NO renderizar» de la guía, que deja solo dos casos sin widget:
  una explicación conceptual y un dato único.
- **El aviso de umbral extremo en la pestaña de `widgets` es permanente** (D6). Se acepta como
  información veraz, no como ruido.
- **El fallo abierto inyecta ahora el fragmento de `widgets`** además de sus herramientas: es el
  comportamiento correcto y se fija con un test de regresión.

## Migration Plan

Una migración, `20261101000001_widgets_always_on.sql`, idempotente y quirúrgica:

1. `DELETE FROM settings WHERE key = 'ROUTER_THRESHOLD_WIDGETS' AND value = '0.20'` — retira el
   override sembrado; el valor efectivo pasa al compilado `0`.
2. `UPDATE settings SET value = '<guía nueva>' WHERE key = 'SKILL_WIDGETS_PROMPT' AND value = '<guía
   vieja verbatim>'` — reescribe la guía solo si nadie la ha tocado.

Se despliega con el próximo release de la imagen. Rollback = revertir el commit: el umbral vuelve a
`0.20` por el valor compilado y la guía a la anterior. No hay estado que deshacer, salvo la fila de
settings, que la migración inversa puede reponer.

## Open Questions

- Ninguna bloqueante. El aviso permanente de umbral extremo en la pestaña de `widgets` (D6) queda
  como mejora de UI posterior, si al usuario le molesta.

## La guía nueva (texto exacto)

Este es el texto que la migración `20261101000001_widgets_always_on.sql` sembrará en
`settings.SKILL_WIDGETS_PROMPT`. Se conserva la **primera línea** —`# Instrucciones de Interfaz y
Widgets Interactivos`— porque es el `prompt_heading` del catálogo (`src/orchestrator/skills.rs`) y la
señal que usa la regla anti-duplicado de `compose_skill_fragments`.

````markdown
# Instrucciones de Interfaz y Widgets Interactivos

Dispones de la herramienta ejecutable `render_widget` para poner un widget interactivo en el chat.

## Regla de oro

Un widget **solo** aparece si ejecutas `render_widget` en esa misma respuesta. NUNCA digas que has mostrado un widget, un formulario, una lista o un mapa si no has ejecutado la herramienta en ese turno: afirmarlo es inventar información.

## La regla de las opciones

Mira lo que estás a punto de escribir. Si van a aparecer **dos o más opciones etiquetadas** (`A`/`B`/`C`, `1`/`2`/`3`) o **una lista de alternativas** para que el usuario elija —en tabla, en viñetas, en prosa o como pregunta—, NO las escribas: renderiza un widget con ellas.

Lo mismo si vas a pedir más de un dato a la vez, proponer un plan con pasos, listar cosas que marcar o situar un lugar en un mapa.

## Los tres widgets

`widget_name` admite exactamente estos tres nombres.

**`QuickForm`** — elegir entre opciones o pedir datos. `fields[].type` es `text`, `textarea`, `number`, `select`, `checkbox` o `slider`; `options` solo con `select`; `min` y `max` solo con `slider`.

```json
{"widget_name":"QuickForm","data":{"title":"Elige una opción","fields":[{"name":"opcion","label":"Opción","type":"select","options":["Actualizar ahora","Esperar a la próxima versión"]}],"submit_label":"Enviar"}}
```

**`Checklist`** — una lista de cosas que marcar. Cada ítem lleva `id` y `label`.

```json
{"widget_name":"Checklist","data":{"title":"Pendientes por despachar","items":[{"id":"1","label":"Responder el correo pendiente"},{"id":"2","label":"Pagar la luz"}]}}
```

**`LocationWidget`** — un mapa con un punto. Necesita `latitude` y `longitude` **reales** (de las herramientas disponibles o del contexto ya conocido); no las inventes.

```json
{"widget_name":"LocationWidget","data":{"title":"Parking del centro","latitude":43.3674,"longitude":-8.4012,"address":"Calle Mayor 1"}}
```

Puedes acompañar el widget con una frase breve de encabezado. Lo que no puedes es repetir en prosa la lista que el widget ya muestra.

## Los dos únicos casos sin widget

La regla de las opciones tiene prioridad sobre todo lo demás. Solo respondes con texto cuando:

1. Es una explicación conceptual que no pide elegir nada ni lleva lista que marcar.
2. Es un dato único y directo: una fecha, un número, un nombre.

Ante la duda, mira lo que ibas a escribir: si hay dos opciones o una lista, no es ninguno de esos dos casos.

## Después de un widget

El usuario contesta con un mensaje como `[widget:Checklist#w3] submit {"checkedIds":["1"]}`. Procesa esa respuesta de inmediato y confirma el resultado en texto, sin volver a renderizar el mismo widget salvo que pida cambiarlo.
````

### Cómo cumple los seis puntos del requisito

El requisito «La guía de widgets SHALL residir en el fragmento de su skill» (`orchestrator/skill-router`)
enumera seis obligaciones. La correspondencia es directa, y es la que el test de la tarea 4.4 debe
comprobar:

| Punto del requisito | Dónde lo cumple la guía |
|:---|:---|
| 1. Declarar que un widget solo aparece si se ejecuta la herramienta, y prohibir afirmar lo contrario | Sección «Regla de oro» |
| 2. Enunciar la regla de las opciones | Sección «La regla de las opciones» |
| 3. Nombrar los tres nombres de `widget_name` con un ejemplo mínimo de `data` cada uno | Sección «Los tres widgets» (tres bloques JSON) |
| 4. Retirar toda autorización genérica de responder en prosa | Sección «Los dos únicos casos sin widget»: la autorización está **acotada** (explicación *sin opciones ni listas*; dato *único*) y la regla de las opciones se declara **prioritaria** |
| 5. Acotar los casos sin widget a dos | Sección «Los dos únicos casos sin widget»: literalmente dos |
| 6. Conservar la regla de procesamiento de la respuesta | Sección «Después de un widget» |

Nota para el revisor: el punto 4 y el punto 5 se sostienen juntos. La guía **sí** menciona los dos casos
en los que se responde con texto, porque el punto 5 los exige; lo que **no** hace es amparar bajo ellos
una lista de opciones, porque la regla de las opciones se declara prioritaria sobre cualquier otra
consideración y los dos casos están acotados con la condición «sin opciones ni listas que marcar».

### Coste

El fragmento viaja en el mensaje de sistema de **todos** los turnos (es la consecuencia de E2′). La
guía anterior medía 1.391 caracteres; esta es más larga porque los tres ejemplos con su `data` son
obligatorios (punto 3 del requisito) y son, además, la pieza que ataca la causa medida: la guía
anterior no nombraba ni uno de los tres widgets y el modelo tenía que deducir el enum y un esquema que
solo vivía en los 600 caracteres de la `description` de la herramienta.

## D7 — La definición de «skill activa» es única (corrección del 1-nov-2026)

La primera implementación de D3 dejó la definición de «activa» **duplicada**: en
`compose_skill_fragments`, que aprendió la regla del umbral cero, y en el filtro de
`read_skill_fragments`, que seguía leyendo solo las claves de `selection.skills`. En los caminos de
fallo abierto la selección va vacía, así que el lector devolvía un mapa vacío y la regla nueva **no
llegaba a actuar nunca**: `render_widget` se exponía sin su guía. Lo destapó la auditoría de solo
lectura del change, no un test: el test que decía cubrirlo construía el mapa de fragmentos a mano y
se saltaba el lector.

El arreglo unifica la definición en un predicado único, `is_active(spec, selection, config)` —no
deshabilitada **y** (en la selección **o** incondicionada)—, del que deriva
`active_skills(selection, config)`. El sitio que lee los fragmentos (`src/orchestrator/agent.rs`)
pasa el conjunto activo, no la selección cruda, y `read_skill_fragments` queda como lector puro de
«estas claves». Con eso, herramientas y guía se rigen por la **misma** regla y no pueden divergir.

## Riesgos residuales de la auditoría (1-nov-2026)

La auditoría encontró el fallo crítico —corregido en D7— y ocho riesgos. Estado de cada uno:

| # | Riesgo | Decisión |
|:--|:---|:---|
| 1 | La guarda del `UPDATE` es **byte a byte** contra el texto sembrado: si el valor guardado difiere en un espacio final o un salto de línea, la guía vieja no se sustituye y el fallo sería silencioso. | **Aceptado.** La guía en producción se verificó byte a byte (lo documenta el diagnóstico del plan) y el idioma de la guarda es el del repo. La primera medición de la batería (6.3) lo delataría: guía vieja ⇒ cero widgets. |
| 2 | El `DELETE` no distingue un `0.20` **sembrado** de un `0.20` fijado **a propósito** por el usuario. | **Aceptado:** son indistinguibles por construcción. Quien quiera volver a condicionar la skill sube el umbral a cualquier otro valor. |
| 3 | La regla de las opciones dispara con etiquetas (`A`/`B`) o con una lista presentada como elección; una enumeración de alternativas **sin etiquetar** y presentada como explicación podría escapar. | **Aceptado con vigilancia.** Es exactamente la rendija que la batería de producción debe medir. Si escapa, se abre la fase determinista (detección de listas en la respuesta). |
| 4 | Si `system_prompt` conservara el encabezado de la guía, la regla anti-duplicado suprimiría la guía **nueva** y el modelo seguiría viendo la vieja. | **Aceptado:** es la red preexistente y el escenario «Un bloque editado no se retira» lo permite. Verificado en producción: `20261008000001` sí retiró el bloque del prompt base. |
| 5 | `route-eval` con `--threshold` repone un default a **toda** skill sin override, incluida `widgets`: mediría una configuración que no está desplegada. | **Fuera de este change.** Es un arnés de desarrollo, no lo cubre la spec. Consecuencia operativa: **no usar `route-eval --threshold` para validar este change**; la validación es la batería de producción (6.3). |
| 6 | La rama `Router` de `exposed_tools` filtra por `selection.skills` y no conoce la regla del umbral cero: hoy coincide porque `select` siempre incluye las skills incondicionadas, pero es una coincidencia. | **Aceptado** (no está roto). Candidato a unificar en `is_active` el día que `Selection` deje de construirse solo desde `select`. |
| 7 | El predicado «incondicionada» (`effective_threshold <= 0.0`) vivía en **dos** sitios y podía divergir. | **Cerrado** en D7: unificado en `is_active`, compartido por la composición y la lectura de fragmentos. |
| 8 | «Umbral `0`» significa «no condicionada» para **cualquier** skill: bajar otra a `0` desde la UI la vuelve siempre activa. | **Aceptado** (D1), con el aviso de umbral extremo en la pestaña de la skill. |
