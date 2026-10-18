# Design

## Evidencia común

Los tres defectos se probaron **contra OpenRouter**, con los prompts y las formas reales de
producción: **22 llamadas**, cada caso reproducido **3 veces** (3/3). La evidencia es reproducible y
está citada por defecto en cada sección. La consulta de contraste en producción es:

```sql
SELECT status, error_message, completion_tokens FROM llm_requests WHERE kind = 'consolidator';
SELECT status, error_message, completion_tokens FROM llm_requests WHERE kind = 'timeline';
```

---

## A. La petición del consolidador no lleva turno de usuario

`call_semantic_chat` (`src/workers/episodic_memory.rs:1092`) manda **un único mensaje `system`**:

```rust
messages: vec![ChatMessage {
    role: "system".into(),
    content: system_content,
    ...
}],
```

**Medido.** `deepseek/deepseek-v4.1-flash`, con esa forma, devuelve `completion_tokens: 1`,
`content: null`, `finish_reason: "stop"` — contenido vacío y **sin error** (3/3). Con un turno de
usuario, el **mismo** prompt devuelve 966–991 tokens de JSON válido con `schema_version: 1` (3/3).

**Descartado tras medir** (no es la causa):

- el razonamiento — falla igual con `enabled:false`, con `effort:high` y sin el campo;
- el presupuesto de tokens — con 16k tampoco;
- el backend de OpenRouter — Makora y CoreWeave dan lo mismo;
- `response_format` — irrelevante.

`qwen` y `mistral` **sí** toleran la forma de solo-`system`. No es que DeepSeek esté roto: es que la
forma de la petición no es una conversación.

**Reproduce la avería real.** 9 y 10 de octubre: **54 filas `kind='consolidator'` con
`completion_tokens = 1`**. `extract_json_object("")` no encuentra objeto → `Err` → y *«a failure
here aborts the pass»* → sin memoria persistente ni hechos esos dos días.

### Las dos formas candidatas

**Forma 1 (recomendada): conservar el prompt como `system` y añadir un turno `user` corto.**
El prompt configurable sigue íntegro en el rol `system`; se añade un mensaje `user` fijo, no vacío
(p. ej. «Consolida el estado según las reglas del sistema y devuelve solo el JSON.»). El contenido
dinámico (`{{ ESTADO_ACTUAL }}`, `{{ BLOQUE_DE_MENSAJES }}`) sigue sustituido dentro del prompt.

**Forma 2: repartir las instrucciones en `system` y el contenido dinámico en `user`.**
Rompe la unidad del prompt editable: parte de lo que hoy es `consolidator_prompt` pasaría a vivir en
el código, y el usuario ya no controlaría el contenido en un solo sitio.

### Impacto sobre el prompt configurable por el usuario

- **Forma 1:** nulo. El prompt editable no se mueve; la UI de ajustes sigue mostrando y guardando lo
  mismo. El turno de usuario es una constante del código que no se configura.
- **Forma 2:** alto. Habría que decidir qué parte del prompt se edita y en qué rol, y el reparto
  dinámico dejaría de ser editable.

**Recomendación:** **Forma 1**, por ser la menos invasiva. No se rediseña el sistema de plantillas:
no hace falta mover `{{ ESTADO_ACTUAL }}` ni `{{ BLOQUE_DE_MENSAJES }}` fuera del `system`.

**Duda registrada:** el turno de usuario óptimo para DeepSeek (texto y longitud) se ha medido como
suficiente con un mensaje corto, pero **no** se ha barrido el espacio de variantes. Si al implementar
aparece un fallo de forma, añadir un `user` breve y explícito («devuelve el JSON») debería bastar;
se anota como punto a verificar en el RED.

---

## B. El prompt sembrado del extractor se contradice

El valor sembrado de `timeline_prompt` dice, en sus reglas:

> - Si el lote no contiene ningún hecho, devuelve una **lista vacía**.

y el formato de salida exige:

> ```json
> {"events": [{"source": 3, "category": "sport", "fact": "..."}]}
> ```

**Medido.** `mistral` **obedece la prosa** y emite `[]` (3/3 sobre el lote real), que el parser, que
espera el objeto `{"events": [...]}`, rechaza como `NotJson`. Es la causa del **35 % de los errores
del timeline** en producción. `DeepSeek` y `qwen` obedecen el formato y no caen en la trampa.

El respaldo del código (`DEFAULT_TIMELINE_PROMPT_FALLBACK`, `src/workers/episodic_memory.rs:155`) **no**
contiene la contradicción: dice «Formato de salida (solo JSON): `{"events": [...]}`» y no menciona la
lista vacía. **Solo el valor sembrado** está mal.

### El arreglo es una migración y respeta la edición del usuario

El arreglo toca el **valor sembrado**, así que necesita una **migración nueva**. Hay que seguir el
**idioma establecido** del repo para no pisar un prompt editado por el usuario: se actualiza
`WHERE key = 'timeline_prompt' AND value = '<texto anterior exacto>'`, y se rellena con
`INSERT ... WHERE NOT EXISTS` si la clave no está. El precedente literal es
`migrations/20261011000001_timeline_fact_integrity.sql` (bloque 4a).

- **Texto anterior exacto** (el que contiene la frase de la lista vacía): el valor sembrado por
  `20261011000001_timeline_fact_integrity.sql`, líneas 39–55.
- **Existe una UI de edición del prompt** (`consolidator-prompt-ui`, prompt del extractor en
  ajustes): por eso la migración **solo** reemplaza si el valor es todavía el sembrado.

**Duda registrada:** la migración nueva debe reproducir el texto anterior **carácter a carácter**
(comillas incluidas) en el `WHERE`. Cualquier desviación (un espacio, una tilde) hace que no coincida
y no actualice nada, silenciosamente. Se anota como riesgo a comprobar con un test de migración
(«tras migrar, un `settings.timeline_prompt` sin editar contiene `{"events": []}`»).

---

## C. Una validación fallida se registra como `success`

La fila de `llm_requests` del consolidador se escribe en `call_semantic_chat` **después** de
`llm_provider.chat(...)` y antes de validar el contenido, y **siempre** con `status = "success"`
cuando el transporte no falló:

```rust
Some(duration_ms),
"success",
None,
...
```

**Medido.** Las 54 respuestas vacías de octubre (mismo `completion_tokens = 1`) salieron como
`status = 'success'`; nadie se enteró. El fallo solo vivía en un `tracing::warn!`.

El arreglo es que la fila refleje la **validación posterior**: `status = 'error'` y el motivo en
`error_message` cuando el contenido no valide. Se apoya en el requisito
`### Requirement: Un fallo del consolidador SHALL registrar el diagnóstico del contenido`
(`openspec/specs/workers/spec.md:534`), que **no se modifica**: define qué va en el motivo (longitud
y preview); este change define **dónde** queda ese motivo (la fila de `llm_requests`).

**Duda registrada:** hoy la fila se escribe una sola vez, con el resultado del transporte. Cambiarla
para reflejar la validación implica mover/escribir la fila **después** de validar. Si el diseño
prefiere **dos** filas (una de transporte y una de validación) en vez de una sola corregida, es una
decisión de implementación; se anota para que el usuario la resuelva si no hay preferencia clara. La
recomendación es **una sola fila** por llamada, con el `status` y el `error_message` finales.

---

## Decisión resuelta: un `[]` suelto es un fallo de forma (Alternativa 2)

El change archivado `timeline-extractor-resilience` tolera hoy un `[]` suelto como **cero hechos**
(`status = 'success'`, sin fila en `timeline_events`). Con la frase del prompt arreglada (defecto B),
ese caso debería desaparecer; pero mientras un modelo pueda seguir emitiendo `[]`, tolerarlo tiene un
coste: **apaga el reintento** que podría recuperar los hechos que el modelo no emitió. Si `[]` se
acepta como «cero hechos», la extracción no vuelve a preguntar y el lote se marca `is_indexed = 1`
igual: lo que el modelo no emitió **se pierde**.

### Alternativa 1 — Tolerar `[]` como cero hechos (statu quo)

- **Coste:** sigue apagando el reintento; un `[]` espurio se pierde en silencio.
- **Beneficio:** no genera ruido de error; coherente con el change anterior.

### Alternativa 2 — Tratar `[]` como **fallo de forma** (reintentar)

- **Coste:** un lote legítimamente sin hechos se reintentaría una vez de más (una llamada extra) y,
  si el modelo insiste en `[]`, terminaría con `status = 'error'` en `llm_requests`.
- **Beneficio:** recupera los hechos que el modelo no emitió en el primer intento; el error deja de
  ser mudo.

**Recomendación: Alternativa 2**, condicionada a que el defecto B (prompt arreglado) reduzca `[]` a
la rareza esperada. El prompt pide explícitamente `{"events": []}` para el lote sin hechos, así que
un `[]` suelto ya no es una respuesta «correcta según el prompt»: es una desobediencia que merece
otro intento. Si el usuario prefiere no tocar la tolerancia actual, se queda la Alternativa 1.

> **Resuelta el 2026-10-12 por decisión del usuario: Alternativa 2.** Un `[]` suelto se trata
> como fallo de forma y se reintenta; la tarea 4.4 retira la tolerancia del array vacío que
> introdujo el change archivado `timeline-extractor-resilience`.

---

## No-goals

- No se cambia la configuración de modelos (`SEMANTIC_MODEL`, `TIMELINE_MODEL`, `COLLAPSE_MODEL`,
  `MEMORY_MODEL`): con el turno de usuario, DeepSeek vuelve a ser viable; elegir modelo es una
  decisión de **coste**, no de viabilidad, y no entra en este change.
- No se toca `is_indexed`, ni `docker-compose.prod.yml`, ni el frontend.
- No se rediseña el sistema de plantillas con `{{ ESTADO_ACTUAL }}` / `{{ BLOQUE_DE_MENSAJES }}`
  sin justificarlo.

## Dudas registradas (resumen)

1. **A:** el texto exacto del turno de usuario para DeepSeek no se ha barrido; se verifica en el RED.
2. **B:** la migración debe copiar el texto anterior **carácter a carácter** en el `WHERE`; un test de
   migración lo comprueba.
3. **C:** una fila de `llm_requests` por llamada, con `status`/`error_message` finales, frente a dos
   filas (transporte + validación). Recomendada: una sola.
4. ~~**Decisión abierta:** tolerar `[]` (statu quo) frente a tratarlo como fallo de forma y reintentar.~~
   **Resuelta el 2026-10-12: Alternativa 2** — un `[]` suelto es un fallo de forma y se reintenta (ver
   «Decisión resuelta»). La tarea 4.4 retira la tolerancia del array vacío.
