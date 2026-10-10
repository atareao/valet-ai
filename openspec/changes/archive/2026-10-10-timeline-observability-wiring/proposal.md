# Change: Timeline — cableado en observabilidad y ajustes

## Why

La feature del timeline (PR #169, v0.11.0) dejó el backend completo —tabla `timeline_events`, Capa D
del worker, tres herramientas, skill y rol de generación— pero **no tocó las cuatro listas cerradas**
que gobiernan lo que la aplicación *muestra*:

- `StatsRepo::background_summary` arma su resultado a partir de la lista fija
  `["router", "archivist", "consolidator", "collapse"]`, así que las filas de `llm_requests` con
  `kind='timeline'` —que el extractor **sí** escribe— se descartan al mapear: el timeline no aparece
  como proceso de fondo.
- `StatsRepo::db_sizes` recorre un array fijo de once tablas que no incluye `timeline_events`, así
  que la tabla no sale en «Database Sizes».
- La pestaña «Prompts» de SettingsDialog no lista `settings.timeline_prompt`, aunque la migración
  `20261010000004` lo siembra y el extractor lo lee: el prompt existe y es editable por API, pero
  **no hay UI para verlo ni cambiarlo**.
- La pestaña «Generación» no lista el rol del timeline, y la migración de defaults siembra doce
  claves, no quince: el rol usa sus defaults en duro (`0.2`, `off`, `2048`) sin poder ajustarse.

El balance es una feature que funciona y es invisible: no se puede ver su consumo, ni diagnosticar un
fallo del extractor, ni ajustar su prompt o sus parámetros.

Además, el uso de la app **en producción tras Traefik** muestra que el flujo SSE llega en bloque en
lugar de gotear. La app **sí** emite `chunk` de forma incremental (medido: 14 eventos repartidos en
~300 ms, también con `Accept-Encoding: gzip, br`), pero el stream no declara que **no debe
transformarse**, que es lo que un proxy intermedio respeta para no acumular ni comprimir la
respuesta.

## What Changes

- **`background_summary` pasa de cuatro a cinco orígenes**: `timeline` entra en la lista canónica y
  en la tarjeta «Procesos de fondo», con etiqueta legible.
- **`db_sizes` incluye `timeline_events`**: de once a doce tablas contadas.
- **La pestaña «Prompts» gana una sub-pestaña «Timeline»** para `settings.timeline_prompt`.
- **La pestaña «Generación» gana el rol «Línea temporal»**, con sus tres campos, y una **migración
  aditiva e idempotente** que siembra `GENERATION_TIMELINE_*` con los defaults que hoy usa en duro.
- **El stream SSE declara su naturaleza no transformable**: `Cache-Control: no-cache, no-transform` y
  `X-Accel-Buffering: no`.

## Impact

### Specs modificadas

- `db/repos`: `StatsRepo::db_sizes` (doce tablas), `StatsRepo::background_summary` (cinco orígenes) y
  los parámetros de generación (cinco roles, quince claves).
- `frontend`: la tabla de tamaños, la tarjeta de procesos de fondo, la pestaña «Prompts» (cinco
  sub-pestañas) y la pestaña «Generación» (cinco roles), en sus dos requisitos.

### Specs nuevas

- `stream-route`: las cabeceras del stream.

### Código

- `src/db/repos/stats.rs` (dos listas), `src/routes/stream.rs` (cabeceras),
  `frontend/src/components/stats/BackgroundCard.tsx` (etiqueta) y
  `frontend/src/components/SettingsDialog.tsx` (prompt y rol). Una migración nueva en `migrations/`.

### Datos

- Migración nueva que siembra las tres claves `GENERATION_TIMELINE_*` sin sobrescribir valores
  existentes. No se reconstruye ninguna tabla.

### No-objetivos

- **No se toca la Capa D ni el extractor**: ya escriben `kind='timeline'` y ya funcionan.
- **No se añaden rutas de API**: todo va por `GET`/`PUT /settings` y los endpoints de estadísticas.
- **No se cambia la configuración de Traefik**: vive fuera de este repositorio. Aquí solo se hace que
  el stream sea explícito sobre sí mismo; si el proxy sigue acumulando, se corrige allí.
- **No se renombra `kind='timeline'`** ni la skill.
