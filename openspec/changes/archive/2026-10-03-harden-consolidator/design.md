# Design: harden-consolidator

## Contexto

El consolidador (Capa C) hace dos llamadas al LLM: la consolidación inicial (que puede abortar la
pasada) y, si el estado supera el presupuesto, una compresión (que nunca aborta). El fallo
observado está en la inicial.

## Causa raíz (evidencia)

- `llm_requests` (13:35:48): `completion_tokens=2048`, `reasoning_tokens=2048` — el `max_tokens`
  entero se gastó en razonamiento.
- Log del provider: `content_len=0`.
- `extract_json_object("")` → `None` → `ConsolidationError::Invalid` → aborta la pasada.

Con `reasoning: off`, el banco de pruebas devolvió JSON válido en el 100% de las llamadas. Es un
fallo de configuración del rol, no del parser.

## Decisión 1 — `GENERATION_SEMANTIC_REASONING = off`

El consolidador es una transformación estructurada; el razonamiento no aporta y puede consumir todo
el presupuesto. Se alinea con `COLLAPSE` y `MEMORY`, que ya usan `off`.

- Default en `src/generation.rs`: `GenerationRole::Semantic => (0.1, "off", 2048)`.
- Migración nueva: `UPDATE settings SET value='off'
  WHERE key='GENERATION_SEMANTIC_REASONING' AND value='low'` (respeta `medium`, etc.).
- No se toca `max_tokens` (2048 basta una vez que no se paga razonamiento).

## Decisión 2 — Prompt V8

Comparado empíricamente (banco de pruebas con lotes reales y escenarios de identidad, contradicción,
preferencias, reglas, no-invención y lote verboso):

- V0 (sembrado): captura reglas 3/5.
- V5 (taxonomía cruda, esqueleto completo): 26/30 esquema, contradicción 3/5, tokens inflados.
- V6 (taxonomía + migración): 24/30, reglas 0/5.
- V7 (taxonomía en prosa): 48/48 pero sobre-extrae (secciones vacías, huso horario inferido,
  `system_rules` redundantes con el perfil) y gasta hasta 389 tokens.
- **V8 (refinado): 48/48, 8/8 en todos los escenarios, 118–165 tokens en el lote verboso.**

V8 añade sobre V7: incluir solo secciones con contenido, no inferir datos no afirmados, y
`system_rules` limitado a instrucciones explícitas al asistente (el trato/idioma/términos a evitar
van a `communication_style`).

Prompt V8 (texto final):

```
System: Eres el consolidador de memoria persistente de Valet. Analizas una conversación reciente entre el usuario y su asistente, y actualizas de forma acumulativa el Perfil de Usuario y las Reglas de Comportamiento.

Devuelve EXCLUSIVAMENTE un objeto JSON válido, sin bloques de código ni texto adicional:
{
  "schema_version": 1,
  "user_profile": { },
  "system_rules": [ ]
}

Organiza user_profile con estas secciones, incluyendo ÚNICAMENTE las que tengan contenido real (no emitas secciones vacías):
- identity: nombre, idioma y ubicación (solo si se afirman explícitamente).
- preferences_and_tastes: objeto con communication_style (tono, detalle, trato tú/usted, idioma, términos a evitar), technology_and_tools, lifestyle_and_leisure y dislikes_and_dealbreakers (lo que detesta o evita).
- lifestyle_and_routines: horarios, hábitos y eventos recurrentes.
- productivity_and_workflow: metodologías y autonomía delegada.
- interests_and_knowledge: proyectos activos y temas de interés.
- relationships_and_entities: personas, proyectos o entidades clave.

Reglas de consolidación:
- Filtro de permanencia: guarda solo preferencias estables; ignora lo efímero de un turno.
- Registra tanto lo que le gusta como lo que rechaza.
- Sobrescritura: ante contradicción prevalece lo nuevo; elimina el dato antiguo.
- No dupliques: si un dato ya está en user_profile, no lo repitas en system_rules ni como regla equivalente.
- system_rules: SOLO instrucciones explícitas del usuario dirigidas al asistente, en forma imperativa y atómica. El trato, el idioma o los términos a evitar pertenecen a communication_style, no a system_rules.
- No inventes ni infieras: no añadas datos (p. ej. huso horario) que la conversación no afirme con claridad.

Estado persistente actual:
{{ ESTADO_ACTUAL }}

Bloque de mensajes:
{{ BLOQUE_DE_MENSAJES }}
```

Conserva los dos placeholders exigidos por la UI y el marker `consolidador de memoria persistente`
(clasificación de stats).

Siembra: migración nueva que actualiza `consolidator_prompt` **solo si el valor es exactamente el
default anterior** (preserva ediciones del usuario). La plantilla de respaldo en código se actualiza
a V8.

## Decisión 3 — Reintento único

En `consolidate_state`, la consolidación inicial se intenta hasta dos veces. Se reintenta cuando el
contenido está vacío, no es JSON objeto o no pasa `validate_payload`. Si el segundo intento también
falla, se devuelve `ConsolidationError::Invalid` (aborta la pasada, como hoy). El reintento NO se
aplica a la compresión, que ya degrada sin abortar.

## Decisión 4 — Diagnóstico

El error de extracción incluye `content_len` y un preview de hasta ~200 caracteres, para no depender
del log del provider.

## Decisión 5 — Presupuesto de memoria 800

V8 produce estados más ricos (secciones de taxonomía) que el prompt sembrado. Con 500 tokens el
estado se comprimiría antes de tiempo, perdiendo detalle. Se sube el default a 800 (techo 1600).

- Constante `PERSISTENT_MEMORY_BUDGET_TOKENS_DEFAULT = 800` en `src/persistent_memory.rs`.
- Migración nueva: `UPDATE settings SET value='800'
  WHERE key='PERSISTENT_MEMORY_BUDGET_TOKENS' AND value='500'` (respeta otros valores).
- Usuarios pueden seguir editándolo desde Ajustes → Memoria → Persistente.

## Alternativas descartadas

- **Subir `max_tokens`:** no garantiza nada con un modelo que puede razonar hasta el límite, y
  encarece.
- **Solo reintento sin `off`:** oculta la causa y duplica coste en el caso común.
- **Migrar el estado a la taxonomía:** aceptado como fuera de alcance por decisión del usuario.
- **Subir `max_tokens`:** se mantiene 2048; con `reasoning off` es suficiente y no encarece.

## Riesgos

- Un usuario que haya personalizado `consolidator_prompt` conservará su versión (la migración solo
  pisa el default exacto). Si quiere V8, lo pega desde Ajustes → Prompts.
- El prompt V8 puede dejar el estado algo más grande que el sembrado; el presupuesto (500) y la
  compresión siguen gobernando el tamaño.
