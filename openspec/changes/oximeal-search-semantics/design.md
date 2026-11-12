# Design: oximeal-search-semantics

## Decisión 1 — La búsqueda por palabras vive en Valet, no en OxiMeal

**Contexto.** OxiMeal busca por subcadena contigua (FTS5 `trigram`, documentado y deliberado en su
`docs/search.md`). El agente razona en palabras.

**Alternativas descartadas.**
- Cambiar OxiMeal: fuera de alcance; su semántica es deliberada y está documentada.
- Solo documentar y confiar en el prompt: la lección de `widgets-always-on` es que una guía
  prescriptiva sola no cambia el comportamiento del modelo de forma fiable.

**Decisión.** `OxiMealService::search_recipes` parte la consulta en términos y combina en Valet.

## Decisión 2 — Intersección primero; si está vacía, por número de coincidencias

**Alternativas descartadas.**
- Intersección estricta: con el tope de 10 resultados por término, un término común puede dejar
  fuera del top-10 a la receta buscada, la intersección se vacía y el agente vuelve a bloquearse.
- Unión pura (OR): devuelve ruido y pierde precisión.

**Decisión.** Contar coincidencias por `id` y ordenar por número de coincidencias descendente. La
intersección (coincidencia con todos los términos) queda arriba; si no hay ninguna completa, el
agente recibe igualmente los mejores parciales en vez de nada.

## Decisión 3 — Peticiones concurrentes y acotadas

Una petición por término, lanzadas en paralelo, con un tope de términos (constante) para acotar
latencia y carga. Los términos sobrantes se ignoran sin fallar. La operación sigue siendo de solo
lectura, así que la concurrencia es segura.

## Decisión 4 — El aviso accionable es un mensaje de la herramienta

Cuando no hay coincidencias, la herramienta devuelve texto explicativo (mismo patrón que el `400`
legible del change anterior), no una lista vacía sin explicación.

## Consecuencias

- Sin cambios en OxiMeal, en los endpoints, en los ajustes ni en el frontend.
- El tope de 10 del contrato de `search_recipes` se mantiene.
- Riesgo aceptado: términos muy cortos (p. ej. `con`, `de`) ensucian los niveles de coincidencia
  parcial; el primer nivel (coincidencia con todos) sigue siendo correcto.
