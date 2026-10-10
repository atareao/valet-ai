# Changelog
## [0.12.0] - 2026-10-11

### Bug Fixes

- *(stream)* El SSE declara no-transform y no-buffering para proxies

### Documentation

- *(plan-003)* Tema 5 aparcado (resiliencia de workers) con su change
- *(openspec)* Change timeline-observability-wiring (cableado en stats y ajustes)
- *(openspec)* Archiva timeline-observability-wiring (fusiona los deltas en specs/)
- *(openspec)* Archiva email-skill
- *(openspec)* Cierra la tarea 4.4 (PR y merge) de los changes archivados

### Features

- *(stats)* El timeline sale como proceso de fondo y en Database Sizes
- *(frontend)* Rol y prompt del timeline en ajustes, y su etiqueta en estadísticas
- *(services)* Cliente de la API de apimail
- *(tools)* Las cuatro herramientas de correo
- *(skills)* La skill email en el catálogo y en el registro
- *(frontend)* URL y API key de apimail en la pestaña API Keys

### Miscellaneous Tasks

- *(just)* Receta push para publicar la imagen en el registro
- *(db)* Siembra las claves GENERATION_TIMELINE_* del rol timeline
- *(db)* Migración de la skill email y la conexión con apimail
## [0.11.0] - 2026-10-10

### Bug Fixes

- *(agent)* La marca de tiempo del historial se limita a los turnos del usuario

### Documentation

- *(plans)* Crear plans/ y documentar la nomenclatura PLAN-XXX
- *(plans)* Cerrar PLAN-002 (casillas ✅, Bloque D como decisión)
- Descartar el autoarranque del host y retirarlo de planes
- *(plans)* Renombrar roadmap a PLAN-004-PENDING y abrir PLAN-003
- *(plans)* PLAN-003 tema 1 — selección por skills
- *(openspec)* Propuesta skill-selection (selección por skills)
- *(plans)* Cerrar el Tema 1 de PLAN-003
- *(agents)* Excepción de reordenación en la numeración de planes
- *(openspec)* Tema 2 (conciencia temporal) — plan y change proposal
- *(plans)* Cerrar el Tema 2 de PLAN-003
- *(openspec)* Change temporal-stamp-user-only
- *(openspec)* Change strava-running y Tema 3 del PLAN-003
- *(plan-003)* Tema 3 cerrado (skill de running / Strava)
- *(openspec)* Change strava-diagnostics
- *(plan)* Tema 4 cerrado (strava-diagnostics)
- *(openspec)* Change activity-timeline y plan PLAN-005
- *(plan)* PLAN-005 cerrado y referenciado con el PR #169

### Features

- *(skills)* Selección por skills, habilitación y umbral por skill
- *(agent)* Conciencia temporal — reloj por turno y marcas en el historial
- *(strava)* Skill de running sobre la API de Strava (solo lectura)
- *(strava)* Errores accionables y sondeo real de la conexión
- *(strava-ui)* Probar conexión, scope visible y aviso de revocación
- *(timeline)* Hechos cronologicos, Capa D, herramientas y skill

### Miscellaneous Tasks

- *(strava)* Variables de entorno en templates y compose
- *(timeline)* TIMELINE_MODEL mecanico en las plantillas
- *(compose)* Reenviar TIMELINE_MODEL al contenedor

### Testing

- *(skills)* Evitar clics por sub-pestaña en el listado de skills
## [0.10.0] - 2026-10-09

### Bug Fixes

- *(workers)* Wire collapse threshold, backpressure, episodic cooldown and stats
- *(just)* Sondear el healthcheck por 127.0.0.1 y no por localhost
- *(just)* Corregir el escape de llaves en los formatos de podman inspect
- *(just)* Forzar la recreación del contenedor al desplegar
- *(server)* Acotar solo el drenado, no el servidor entero
- *(test)* Declarar el test huérfano de last-call y darle valor real
- *(test)* Neutralizar el message de antd que dejaba timers vivos al desmontar
- *(ui)* El error de EventModal debe salir por su messageApi, no por el message estático
- *(docker)* Compilar sqlite-vec en musl aliando u_intN_t
- *(memory)* No abortar por tamaño, cota inferior de tokens y limpieza (Capa C)
- *(memory)* Hacer atómica la escritura y reutilizar el clasificador de presupuesto
- *(ui)* Usar el presupuesto editado y cerrar hallazgos del panel
- *(tools)* Search_places usa searchText sin radius y searchNearby con radius
- *(tools)* Timeout HTTP de 30 s en la tool weather
- *(tools)* Search_places usa searchText con locationBias en vez de searchNearby
- *(tools)* Valida y acota el radius de search_places y retira included_type
- *(widgets)* Usar tiles de OpenStreetMap en LocationWidget
- *(docker)* Compilar solo la lib al calentar la caché de dependencias

### Documentation

- *(openspec)* Cerrar y archivar el change con su resultado real
- *(openspec)* Registrar la publicación real y corregir las desviaciones
- *(openspec)* Cerrar las casillas del change de los formatos de podman
- *(openspec)* Archivar los dos changes y consolidar sus specs
- *(openspec)* El Purpose de las dos specs archivadas deja de ser un placeholder
- Dejar escritas las decisiones aplazadas y cerrar los ficheros sueltos
- *(openspec)* Arreglar 10 comandos de verificación del plan
- *(openspec)* Aprobar el change proposal de memoria episódica
- *(openspec)* Marcar las tareas completadas del plan
- *(openspec)* Registrar en el plan el arreglo del build en musl
- *(openspec)* Archivar el change de memoria episódica
- *(openspec)* Aprobar el change de la fecha de los hechos en la memoria
- *(openspec)* Cerrar el plan del change de fechas
- *(openspec)* Archivar el change de las fechas de la memoria
- *(openspec)* Cerrar el plan del retiro de process_message
- *(openspec)* Archivar el change del retiro de process_message
- *(openspec)* Cerrar el plan del reagrupado del prompt
- *(openspec)* Archivar el change del mensaje único de sistema
- *(openspec)* Proponer la Capa C (memoria persistente) — núcleo
- *(openspec)* Precisar techo, fallo de compresión y marca de la Capa C
- *(openspec)* Archivar el change de la memoria persistente (Capa C)
- *(openspec)* Proponer el estimador determinista de tokens JSON
- *(openspec)* Precisar el estimador y cerrar el plan
- *(openspec)* Archivar el estimador de tokens JSON
- *(openspec)* Proponer la interfaz de la memoria persistente
- *(openspec)* Cerrar el plan de la interfaz de memoria persistente
- *(openspec)* Archivar la interfaz de memoria persistente
- *(openspec)* Proponer el prompt del consolidador en la pestaña Prompts
- *(openspec)* Archivar el prompt del consolidador
- *(openspec)* Proponer generation-params (temperatura, razonamiento y tokens por rol)
- *(openspec)* Archivar generation-params
- *(openspec)* Proponer settings-dialog-layout (sub-pestañas en Generación y diálogo más ancho)
- *(openspec)* Archivar settings-dialog-layout
- *(openspec)* Proponer settings-memory-tabs (sub-pestañas Episódica/Persistente en Memoria)
- *(openspec)* Archivar settings-memory-tabs
- *(openspec)* Proponer user-name-in-prompt (inyectar el nombre del usuario en el prompt)
- *(openspec)* Archivar user-name-in-prompt
- *(openspec)* Proponer remove-meals-habits-contacts (podar tools)
- *(openspec)* Archivar remove-meals-habits-contacts
- *(openspec)* Proponer align-tools-registry (registry, enabled y fantasma)
- *(openspec)* Archivar align-tools-registry
- *(openspec)* Proponer tool-approval-hitl (HITL y permiso por operación)
- *(openspec)* Archivar tool-approval-hitl
- *(openspec)* Proponer fix-search-places-text-query
- *(openspec)* Proponer fix-weather-http-timeout
- *(openspec)* Archivar fix-search-places-text-query y fix-weather-http-timeout
- *(openspec)* Proponer harden-consolidator
- *(openspec)* Archivar harden-consolidator
- *(openspec)* Proponer fix-search-places-radius-bias
- *(openspec)* Archivar fix-search-places-radius-bias
- *(openspec)* Proponer settings-tools-tab
- *(openspec)* Archivar settings-tools-tab
- *(openspec)* Proponer fix-search-places-radius-validation
- *(openspec)* Archivar fix-search-places-radius-validation
- *(openspec)* Proponer characterize-legacy-tools
- *(openspec)* Archivar characterize-legacy-tools
- *(openspec)* Proponer characterize-legacy-db
- *(openspec)* Archivar characterize-legacy-db
- *(openspec)* Archivar frontend-tools-scroll
- *(openspec)* Archivar interactive-widgets
- *(openspec)* Archivar render-widget-schema
- *(openspec)* Archivar widget-prompt-guidance
- *(openspec)* Archivar location-widget
- *(openspec)* Archivar fix-location-tiles
- *(openspec)* Archivar persist-rendered-widgets
- *(openspec)* Proposal del change oidc-auth
- Documentar las variables de entorno de auth y sus defaults fail-closed
- *(openspec)* Marcar tareas de verificación 6.1-6.3 del change oidc-auth
- *(openspec)* Archivar oidc-auth
- *(openspec)* Archivar prod-traefik
- *(openspec)* Marcar la verificación 6.4 del OIDC y el archivado 7.2
- *(openspec)* Cerrar persist-rendered-widgets (PR #130, verificación y archivado)
- *(openspec)* Archivar login-dark-ux
- *(openspec)* Archivar lazy-app-bundle
- *(openspec)* Archivar skill-router
- *(openspec)* Archivar skill-router-tuning
- *(openspec)* Archivar route-eval-fidelity
- *(openspec)* Archivar route-eval-campaign-support
- *(openspec)* Archivar morning-briefing-criteria
- *(openspec)* Archivar router-enabled-by-default
- *(openspec)* 📋 propuesta llm-usage-by-origin (marcar los 5 orígenes de llamada LLM)
- *(openspec)* Marca completadas las tasks de llm-usage-by-origin (incl. E2E)
- *(openspec)* 📦 archiva llm-usage-by-origin (fusiona los deltas en specs/)

### Features

- *(pwa)* Serve web icons and PWA manifest from frontend/public
- *(rag)* Wire RAG pipeline with unified embeddings
- *(chat)* Avatares de Valet y del perfil en las burbujas
- *(server)* Cierre ordenado ante SIGTERM y SIGINT
- *(db)* Habilitar sqlite-vec y no arrancar sin ella
- *(db)* Migrar vec_memory a tabla virtual vec0 con escritura binaria
- *(memory)* KNN con umbral y decaimiento, y los cuatro mandos en settings
- *(frontend)* Pestaña Memoria con los cuatro mandos
- *(memory)* Inyectar la memoria episódica en cada mensaje
- *(memory)* Reseteo de la fuente para reconstruir el índice
- *(memory)* Fechar los hechos y medir la antigüedad por ellos
- *(orchestrator)* Reunir el prompt en un único mensaje de sistema
- *(memory)* Núcleo de la Capa C — migración, repositorio, config y esquema
- *(worker)* Consolidar la Capa C en una pasada con presupuesto y techo
- *(orchestrator)* Inyectar la memoria persistente en el mensaje de sistema
- *(memory)* Estimador determinista de tokens JSON para la Capa C
- *(memory)* API de lectura y edición del estado persistente
- *(ui)* Pestaña de memoria persistente en ajustes
- *(ui)* Editar el prompt del consolidador en la pestaña Prompts
- *(llm)* Parámetros de generación por rol (temperatura, razonamiento y tokens) con UI
- *(ui)* Sub-pestañas por rol en Generación y diálogo de ajustes más ancho
- *(ui)* Agrupar Memoria y Memoria persistente en sub-pestañas Episódica/Persistente
- *(agent)* Inyectar el nombre del usuario en el mensaje de sistema
- *(tools)* Alinear registry, hacer real el toggle enabled y cablear las fantasma
- *(tools)* Aprobación humana real y permiso por operación
- *(consolidator)* No razonar por defecto, prompt V8 y reintento único
- *(frontend)* Pestaña Herramientas para activar/desactivar tools
- *(frontend)* Scroll y altura máxima en la lista de la pestaña Herramientas
- *(tools)* Tool render_widget y evento SSE widget
- *(frontend)* Widgets interactivos (QuickForm y Checklist)
- *(widgets)* Documentar el esquema de data y endurecer la lectura
- *(db)* Anexar la guía de widgets al system_prompt
- *(widgets)* Añadir LocationWidget con mapa y acciones
- *(widgets)* Persistir y reconstruir los widgets al recargar
- *(frontend)* Login, guard de sesión y logout OIDC
- *(infra)* Desplegar el stack de producción tras Traefik
- *(tools)* Descripciones en español y esquemas de function-calling más robustos
- *(ui)* Pantalla de login con fondo oscuro y logo real de Valet
- *(orchestrator)* Enrutado de skills por turno con Jev
- *(stats)* Marcar el origen de cada llamada LLM (kind) y panel de procesos de fondo

### Miscellaneous Tasks

- *(tests)* Eliminar los tests de depuración sin aserciones
- CI en GitHub Actions, publicación en GHCR y deploy verificado
- *(image)* Exigir que la imagen sobreviva al presupuesto de drenado
- *(image)* No construir la imagen si el push solo cambia documentación
- *(infra)* Propagar modelos y knobs del worker en el compose
- *(eval)* Actualizar modelos de colapso y consolidador

### Refactor

- *(workers)* Remove unused proactive workers and their config
- *(ui)* Los avisos pasan a la API contextual de antd (App.useApp)
- *(memory)* Retirar la estrategia RAG y limpiar el código muerto
- *(orchestrator)* Retirar la ruta muerta process_message y registrar el error del stream
- *(stats)* Correcciones de revisión en el marcado de orígenes

### Testing

- Caracterizar la memoria episódica antes de tocarla
- *(memory)* Fijar la aproximación de \uXXXX y de los números
- *(tests)* Tests RED para el endurecimiento del consolidador
- *(consolidator)* Cubrir preview del diagnóstico y no-reintento de transporte
- *(frontend)* RED pestaña Herramientas en Settings
- *(stats)* 🔴 tests RED para el marcado de orígenes LLM (llm-usage-by-origin)
## [0.9.0] - 2026-09-29

### Features

- *(prompts)* Load system prompts from DB via migration

### Miscellaneous Tasks

- Normalize openspec specs, add eslint config and fix flaky test
## [0.8.1] - 2026-09-29

### Documentation

- *(openspec)* Archive fix-tools-used-persistence and cleanup-clippy-dead-deps

### Miscellaneous Tasks

- Fix clippy lints and remove unused dev-dependency
## [0.8.0] - 2026-09-29

### Dependencies

- Upgrade axum 0.8, sqlx 0.9 and adapt breaking changes

### Documentation

- Update READMEs and openspec spec for dependency upgrade
## [0.7.0] - 2026-09-29

### Bug Fixes

- *(calendar)* Make duration optional in get_events with default 24h
- Hora local en vez de UTC y ubicación con reverse geocode inline en orchestrator
- Guardar lat/lon inline antes de lanzar orchestrator, reverse geocode en background
- Stats table headers visibles en dark mode + refresco al abrir modal
- DarkAlgorithm + Table/Header tokens para dark mode correcto
- Stats recording con modelo real, coste, cached_tokens y reasoning_tokens desde OpenRouter
- No emitir StreamEvent::Done hasta que el SSE chunk incluya usage
- Nombres de campo correctos en usage de OpenRouter
- Persist tools_used to DB and fix ToolDef serialization for OpenRouter
- Persist tools_used to DB and fix ToolDef serialization for OpenRouter
- Rewrite agent prompt with expanded personality, rules and examples
- Rewrite agent prompt with expanded personality, rules and examples
- Merge agent prompt rewrite from main

### Documentation

- Update OpenRouter spec and remove archived change proposal
- Rewrite README in English, add Spanish version and .env.example

### Features

- *(openrouter)* Add app identification headers to API calls
- *(stats)* Implement stats dashboard with LLM usage, DB sizes, and retention
- *(ui)* Add Stats button to header bar
- *(memory)* Implement episodic memory system
- Implement stats recording, time/location tools, and worker fixes
- Message timestamp, location and date separators
- Add LastApiCall model, endpoint and stats recording in workers
- Add LastApiCallCard UI and Última llamada tab to dashboard
- Add LastApiCall observability endpoint and UI

### Miscellaneous Tasks

- Rename project from Alfred to Valet
- Rename project from Alfred to Valet

### Refactor

- Reverse_geocode inline con cache, eliminar tokio::spawn

### Styling

- Ubicación completa en segunda línea, eliminar extractCity
- StatsDashboard organizado en pestañas (Resumen/Modelos/Sistema)
## [0.6.0] - 2026-09-26

### Bug Fixes

- *(deps)* Update vite to 8.3.1 and vitest to 5.0.2 to fix 7 Dependabot vulnerabilities

### Features

- Consolidate all pending changes
- Consolidate agenda, calendar UI, and orchestrator fixes
- *(tasks)* Implement GTD task management with Kanban and List views (#13)

### Miscellaneous Tasks

- Remove tsbuildinfo from tracking
## [0.5.0] - 2026-09-25

### Documentation

- Update CHANGELOG for v0.5.0

### Features

- Configurable message page size via settings
- Configurable message page size via settings
- Connect real CollapseWorker with configurable model
- Connect real CollapseWorker with configurable model
- Consolidate all pending changes

### Miscellaneous Tasks

- Bump version to 0.5.0
