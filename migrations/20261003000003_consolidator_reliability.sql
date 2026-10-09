-- Endurecimiento del consolidador de memoria persistente (Capa C).
--
-- 1) El rol Semantic deja de razonar por defecto: con un modelo de razonamiento
--    `low` podía consumir todo `max_tokens` y dejar el contenido vacío. Solo se
--    corrige el valor heredado `low`; cualquier otro valor se respeta.
-- 2) El presupuesto de memoria persistente sube de 500 a 800. Solo se corrige
--    el valor heredado `500`.
-- 3) Se actualiza el prompt sembrado del consolidador al V8 SOLO si sigue siendo
--    exactamente el default anterior (así se preservan personalizaciones).
--
-- Append-only: no se editan migraciones existentes.

UPDATE settings SET value = 'off', updated_at = datetime('now')
WHERE key = 'GENERATION_SEMANTIC_REASONING' AND value = 'low';

UPDATE settings SET value = '800', updated_at = datetime('now')
WHERE key = 'PERSISTENT_MEMORY_BUDGET_TOKENS' AND value = '500';

UPDATE settings SET value = 'System: Eres el consolidador de memoria persistente de Valet. Analizas una conversación reciente entre el usuario y su asistente, y actualizas de forma acumulativa el Perfil de Usuario y las Reglas de Comportamiento.

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
{{ BLOQUE_DE_MENSAJES }}', updated_at = datetime('now')
WHERE key = 'consolidator_prompt' AND value = 'System: Eres el consolidador de memoria persistente de Valet.
Recibes el estado persistente actual y un bloque de mensajes nuevos, y devuelves el estado actualizado.

Devuelve EXCLUSIVAMENTE un objeto JSON con esta forma:
{
  "schema_version": 1,
  "user_profile": { },
  "system_rules": [ ]
}

Conserva la información ya presente y añade o corrige solo lo que los mensajes justifiquen.
No añadas claves de primer nivel distintas de las indicadas.

Estado persistente actual:
{{ ESTADO_ACTUAL }}

Bloque de mensajes:
{{ BLOQUE_DE_MENSAJES }}';
