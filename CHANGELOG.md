# Changelog
## [0.19.0] - 2026-10-25

### Documentation

- *(plans)* Cierra el PLAN-007 (bitácora autónoma)
- *(openspec)* Propone react-tool-guards (tres guardas del bucle ReAct)
- *(openspec)* Archiva react-tool-guards

### Features

- *(orchestrator)* Tres guardas para las llamadas a herramientas del bucle ReAct
- *(frontend)* Los límites del agente, en Ajustes
## [0.18.2] - 2026-10-25

### Documentation

- *(openspec)* Archiva adopt-jsdom-30-1-3

### Miscellaneous Tasks

- *(deps-dev)* Bump @vitejs/plugin-react in /frontend
- *(deps-dev)* Bump vite from 8.3.1 to 8.3.5 in /frontend
## [0.18.1] - 2026-10-25

### Bug Fixes

- *(chat)* La marca de optimista deja de filtrarse a la fila canónica
## [0.18.0] - 2026-10-25

### Bug Fixes

- *(chat)* Evita el corte de respuestas largas y preserva el texto parcial

### Documentation

- *(openspec)* Archiva chat-cross-device-sync y chat-stream-resilience
- *(openspec)* Archiva chat-reasoning-default-off

### Features

- *(chat)* El chat no razona por defecto
## [0.17.0] - 2026-10-24

### Documentation

- *(openspec)* Redacta el change chat-cross-device-sync y su plan

### Features

- *(chat)* Bus de eventos y cursor de orden total para sincronizar el chat
- *(chat)* El frontend se suscribe al bus y recupera la sincronía

### Miscellaneous Tasks

- *(frontend)* Resincroniza static con el build actual
## [0.16.0] - 2026-10-24

### Bug Fixes

- *(llm)* El proveedor expone el motivo de finalización y rechaza una respuesta sin choices
- *(workers)* La consolidación degrada la pasada, dobla el presupuesto al truncar y no colapsa en vacío
- *(bitacora)* El test de la fecha prueba lo que dice y el animo se agrupa
- *(stats)* Calcula las fechas de los tests desde el reloj, no literales

### Documentation

- *(workers)* Los comentarios dejan de hablar de abortar la pasada
- *(openspec)* La spec del proveedor describe la forma plana del motivo de finalización
- *(agents)* El item 6 deja de dar llm-worker-resilience por aparcado
- *(agents)* Registra la medición de «marcar todas» como cerrada
- *(agents)* El índice deja de anunciar observabilidad en el roadmap
- *(plans)* Quita de PLAN-006 la nota que remitía al roadmap de observabilidad
- *(plans)* Abre PLAN-007 para la bitácora autónoma
- *(openspec)* Redacta el change journal-entries
- *(openspec)* Registra la medicion del enrutado y el tope del listado
- *(plans)* Registra el cierre de journal-entries en PLAN-007
- *(openspec)* Redacta el change journal-worker
- *(openspec)* El prompt de la bitacora se edita desde Ajustes
- *(plans)* Registra el cierre de journal-worker en PLAN-007
- *(openspec)* Registra el diseño del publicador de respuestas
- *(openspec)* Cierra la verificación global del publicador de respuestas
- *(openspec)* Redacta el change comment-deletion
- *(openspec)* Rellena el Purpose de frontend/notifications

### Features

- *(bitacora)* La tabla de entradas y su repositorio
- *(bitacora)* La herramienta del agente y su sitio en el enrutado
- *(bitacora)* Los endpoints de la bitácora
- *(bitacora)* La sección de bitácora en la app
- *(bitacora)* El dia local, los ajustes y el prompt del diario
- *(bitacora)* El worker que escribe el diario por su cuenta
- *(bitacora)* La migracion, la visibilidad y el editor del prompt
- *(blog-comments)* Publica las respuestas del blog con reserva idempotente
- *(blog-comments)* Lee el comentario y lo manda a la papelera
- *(blog-comments)* La herramienta del borrado y su sitio en el catálogo

### Miscellaneous Tasks

- *(openspec)* Reanuda llm-worker-resilience y repara sus artefactos
- *(openspec)* Elimina las obligaciones duplicadas del delta de workers
- *(env)* Las plantillas dejan de sembrar modelos que razonan en los roles mecánicos
- *(openspec)* Archiva el change llm-worker-resilience
- *(plans)* Retira el apartado de observabilidad del roadmap v2
- *(openspec)* Archiva el change journal-entries
- *(frontend)* Resincroniza static con el build de la bitacora
- *(env)* La bitacora tiene su modelo por entorno sin razonamiento
- *(openspec)* Archiva el change journal-worker
- *(openspec)* Archiva el change comment-reply-publisher
- *(openspec)* Archiva el change comment-deletion
- *(openspec)* Normaliza DEBE a SHALL en los requirements de infra
- *(frontend)* Resincroniza static con el build actual

### Refactor

- *(llm)* El motivo de finalización se serializa en forma plana y se re-exporta

### Testing

- *(llm)* Rojo del rechazo de una respuesta sin choices
- *(llm)* Rojo del motivo de finalización en el contrato del proveedor
- *(workers)* Rojo de la degradación, el presupuesto doblado y la guarda del colapso
- *(llm)* Rojo del ida y vuelta plano del motivo de finalización
- *(workers)* Cubre los escenarios que la revisión dejó sin aserciones
- *(bitacora)* Rojo del esquema, el modelo y el repositorio
- *(bitacora)* Rojo de la herramienta y del enrutado
- *(bitacora)* Rojo de los endpoints de la bitácora
- *(bitacora)* Rojo de la sección de bitácora
- *(bitacora)* Corrige dos aserciones del rojo de la bitácora
- *(bitacora)* Rojo del dia, los ajustes y el prompt
- *(bitacora)* Rojo del worker y de su fallo
- *(bitacora)* Rojo de la migracion, las estadisticas y el editor
## [0.15.0] - 2026-10-18

### Bug Fixes

- *(push)* Estados de permiso honestos y sin cuelgues en el registro
- *(push)* Validar el punto P-256, normalizar el endpoint y tests honestos
- *(workers)* El extractor del timeline deja de perder hechos
- *(workers)* El consolidador deja de salir hueco y las stats dejan de mentir
- *(notifications-ui)* Atiende la revisión del frontend
- *(frontend)* Interpreta fechas de claves de ingesta como UTC

### Documentation

- *(openspec)* Archivar el change timeline-fact-integrity
- *(openspec)* Archivar el change webpush-notifications
- *(openspec)* Corregir las casillas verificadas del archive de webpush
- *(openspec)* Registrar la verificación de la 5.6 en producción
- *(openspec)* Archivar el change push-delivery-observability
- *(openspec)* Redactar el change notification-ingest
- *(openspec)* Archivar el change notification-ingest
- *(api)* Documenta la API de notificaciones
- *(agents)* Registra la medición aplazada de notifications-inbox
- *(plans)* Cierra PLAN-003 tras completar temas 1-4
- *(plans)* Cierra PLAN-006 tras sustituir «Guardar» por «Abrir»
- *(agents)* Registra la prueba en Android aplazada y añade PLAN-006 al índice
- *(agents)* Registra la prueba en Android de «Abrir» como verificada
- *(agents)* Registra que MCP queda descartado

### Features

- *(push)* Construir la petición WebPush
- *(push)* Enviar con reqwest y clasificar la respuesta
- *(push)* Exponer /api/push (vapid-key, subscribe, unsubscribe)
- *(push)* Worker que entrega los recordatorios vencidos
- *(push)* Service worker y gestor de suscripción
- *(push)* Registrar el service worker sin romper la app
- *(push)* Tab de Notificaciones en Ajustes
- *(push)* Contadores por suscripción y predicado del resumen del ciclo
- *(db)* Migración notification_ingest (tablas notifications e ingest_keys)
- *(notifications)* Repositorio de notificaciones (fase 2.1.2)
- *(push)* Envío masivo y ayudante de host compartido (fase 3)
- *(ingest-keys)* Repositorio de claves de ingesta (fase 2.2.2/2.2.3)
- *(tools)* Herramienta notifications en la skill pendientes (fase 7)
- *(router)* Las criteria y el fragmento de pendientes cubren los avisos entrantes (fase 7.3)
- *(frontend)* Sección de claves de ingesta en la pestaña API Keys (fase 8)
- *(notifications)* Autenticación por token y rutas de ingesta y consumo (fases 4 y 5)
- *(ingest-keys)* Alta, listado y revocación de claves de ingesta (fase 6)
- *(ingest)* Responde 401 con cuerpo JSON y registra el motivo
- *(stats)* Agrega el eje de modelo a la agregación de fondo
- *(stats-ui)* Una fila por rol y modelo, y ventana de 7 días
- *(notifications)* Marca todas las no leídas y pagina por cursor
- *(notifications-ui)* Bandeja de avisos paginada con marca individual y masiva
- *(location-widget)* «Abrir» sustituye a «Guardar» en el LocationWidget

### Miscellaneous Tasks

- *(push)* Regenerar static y guardar el servicio de sw.js
- *(push)* Marcar la verificación de la imagen en musl
- *(push)* Marcar la suite completa verificada
- *(openspec)* Registrar la verificación de la fase 9
- *(openspec)* Archiva el change ingest-auth-diagnostics
- *(openspec)* Archiva el change timeline-extractor-resilience
- *(openspec)* Archiva el change episodic-call-integrity
- *(openspec)* Añade el change stats-role-model-view
- *(openspec)* Archiva el change stats-role-model-view
- *(openspec)* Propone el change ingest-keys-timestamp-utc
- *(openspec)* Cierra las tareas 6.7 y 6.8 de stats-role-model-view
- *(openspec)* Propone el change notifications-inbox
- *(openspec)* Cierra las tareas de notifications-inbox (48/50)
- *(openspec)* Archiva el change notifications-inbox
- *(openspec)* Archiva el change ingest-keys-timestamp-utc
- *(openspec)* Archiva el change location-widget-open-action
- *(frontend)* Resincroniza static con el build actual y quita 7 commits de deriva
- *(openspec)* Cierra la tarea 6.3 de location-widget-open-action

### Other

- Tests de IngestRepo (2.2.1) y registro del módulo

### Refactor

- *(notifications)* Atiende la revisión del backend

### Testing

- *(db)* RED de la migración notification_ingest
- *(push)* RED del envío masivo y del ayudante de host compartido
- *(notifications)* RED del repositorio de notificaciones (fase 2.1.1)
- *(notifications)* RED de la herramienta del agente y su sitio en el catálogo (fase 7.1)
- *(frontend)* Rojo de la seccion de claves de ingesta (8.1)
- *(notifications)* RED de la autenticación y las rutas de ingesta (fases 4 y 5)
- *(ingest-keys)* RED de alta, listado y revocación (fase 6, tarea 6.1)
- *(router)* El fallo abierto cubre las veintinueve herramientas
- *(notifications)* Rojo para marcar todas y paginación por cursor
- *(notifications)* Rojo de ruta y herramienta
- *(notifications-ui)* Rojo de la bandeja de avisos
## [0.14.0] - 2026-10-11

### Documentation

- Registrar el análisis de granularidad y selección de skills
- *(openspec)* Proposal, specs y tasks del change timeline-fact-integrity

### Features

- *(timeline)* Sin duplicados, con fecha real, lugar y grano de acontecimiento

### Refactor

- *(timeline)* Acotar el conflicto a la unicidad y cubrir el lote sin hechos

### Testing

- *(timeline)* RED — contrato de la migración de integridad y de los prompts
- *(timeline)* RED — contrato del extractor (fuente, fecha, lugar, grano y bloque)
- *(timeline)* RED — contrato del repositorio y de las herramientas
## [0.13.0] - 2026-10-11

### Dependencies

- Jsonwebtoken 9 a 11 con el CryptoProvider aws_lc_rs
- Source-map-js 1.2.1 a 1.2.2

### Documentation

- *(openspec)* Change jsonwebtoken-11
- *(openspec)* Archiva jsonwebtoken-11
- *(openspec)* Cierra las tareas del change jsonwebtoken-11
- *(openspec)* Cierra la verificación en producción de v0.12.0
- *(openspec)* Propone la skill rss, lector de feeds Miniflux
- *(openspec)* Cierra las tareas 1.1 a 1.4 del change rss
- *(openspec)* Cierra las tareas 3.1 a 3.6 del change rss
- *(openspec)* Cierra las tareas 4.1 a 4.3 y 5.1 a 5.2 del change rss
- *(openspec)* Archiva la skill rss en las specs
- *(openspec)* Cierra la verificación en CLI del change rss
- *(openspec)* Cierra las tareas 5.4 y 0.1 de los changes archivados

### Features

- *(db)* Siembra los ajustes de la skill rss
- *(rss)* Implementa el cliente de la API v1 de Miniflux
- *(rss)* Añade categories() al cliente y arregla un futuro !Send
- *(rss)* Implementa las cuatro herramientas del feed
- *(rss)* Añade la skill rss al catálogo y registra sus herramientas
- *(settings)* Añade los ajustes de Miniflux a Ajustes

### Testing

- *(db)* Añade en rojo el test de la migración de los ajustes de rss
- *(rss)* Añade en rojo los tests del cliente Miniflux
- *(settings)* Exige los campos de Miniflux en API Keys
## [0.12.0] - 2026-10-10

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
