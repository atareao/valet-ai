# Change: Inyectar el nombre del usuario en el mensaje de sistema

## Why

El nombre del usuario vive en `profiles.name`, es editable desde la pestaña «Perfil» del diálogo
de ajustes y se persiste correctamente. Sin embargo, el orquestador **nunca lo lee**: al componer
la petición al LLM solo usa `settings.system_prompt`, la memoria persistente (Capa C), la memoria
episódica (Capa B) y el contexto del navegador. `process_message_stream` recibe `profile_id` y lo
usa únicamente para el `ContextBuilder` (que lo ignora) y para las estadísticas. El resultado es
que Valet no sabe cómo se llama el usuario y no puede tenerlo en cuenta al responder.

## What Changes

- **Nueva sección de código con el nombre del usuario.** El mensaje de sistema pasa a incluir una
  sección `# USUARIO` con `El nombre del usuario es {name}. Dirígete a él por su nombre cuando sea
  natural, sin repetirlo en cada respuesta.`, compuesta **en código** (no desde un placeholder de
  `settings.system_prompt`), situada **entre el prompt y la memoria persistente**. La guía de uso
  evita tanto olvidar el nombre como repetirlo mecánicamente en cada respuesta; el tono y la
  cortesía siguen viviendo en `settings.system_prompt`.
- **Lectura por `profile_id` en cada petición.** Se añade `ProfilesRepo::get_by_id(pool, id)` que
  devuelve el perfil o `None` sin crear filas. El orquestador lo usa para leer `profiles.name`.
- **Omitida sin rastro cuando no hay nombre real.** La sección se omite si el nombre está vacío tras
  recortar espacios o si es el valor por defecto `Valet User` (perfil recién creado). Si la lectura
  del perfil falla, se loguea un `warn` y se omite la sección sin abortar la petición.

### Orden final del mensaje de sistema

1. Prompt (`settings.system_prompt`).
2. **Nombre del usuario** (nuevo).
3. Memoria persistente (Capa C), si la hay.
4. Memoria episódica (Capa B), si la hay.
5. Fecha, hora y ubicación, si el navegador aporta contexto (sigue cerrando el mensaje).

## Impact

- Afecta: `src/orchestrator/agent.rs` y `src/db/repos/profiles.rs` (más sus tests).
- Specs: `openspec/specs/orchestrator/agent/spec.md` (1 MODIFIED + 1 ADDED) y
  `openspec/specs/db/repos/spec.md` (1 ADDED).
- Sin cambios de API, sin nuevas claves `settings`, sin migraciones, sin frontend: el campo
  «Nombre» de «Perfil» ya existe y ya se persiste vía `PUT /api/profile`.
- Coste: una lectura extra a `profiles` por petición (neglible).
- No toca `docker-compose.prod.yml` ni el despliegue.

### Fuera de alcance

- Inyectar `profiles.preferences` ni el avatar.
- Cambiar el valor por defecto `Valet User` de `ProfilesRepo::get_or_create`.
- Enlazar el `Claims`/JWT de `src/auth.rs` (hoy no está cableado a las rutas): el nombre sigue
  viniendo del perfil único, no de un proveedor de identidad.
