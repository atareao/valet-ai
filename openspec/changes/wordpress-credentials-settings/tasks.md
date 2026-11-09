# Tareas — change wordpress-credentials-settings

Issue de entrada: [#332](https://github.com/atareao/valet-ai/issues/332)

## 1. RED — Tests que fallan primero

- [x] 1.1 Servicio: `Wordpress::resolve_config` lee `blog_base_url`/`blog_service_user`/`blog_app_password` de `settings` (BD en memoria) y NO de variables de entorno; sin usuario/contraseña devuelve un error accionable.
- [x] 1.2 Servicio: sin `blog_base_url` la URL base resuelta es `https://atareao.es`.
- [x] 1.3 Herramienta: con `BLOG_SERVICE_USER`/`BLOG_APP_PASSWORD` definidas pero ajustes vacíos, `blog_comment_reply` falla sin enviar petición (no lee el entorno).
- [x] 1.4 API: `blog_app_password` no aparece en `GET /api/settings` y sí `blog_base_url`/`blog_service_user`.
- [x] 1.5 API: `GET /api/blog/status` → `{configured, base_url, user}` sin la contraseña; `configured` exige usuario y contraseña.
- [x] 1.6 API: `PUT /api/blog/password` con `{"password": "  "}` → `400`; con válida → `204` y queda recortada en `settings`.
- [x] 1.7 API: `GET /api/blog/check` siempre `200`; `ok:false` sin credenciales (sin red), con `401` (sin filtrar la contraseña) y con caída de red; `ok:true` con credenciales válidas.
- [x] 1.8 Migración: siembra `blog_base_url=https://atareao.es`, `blog_service_user=''`, `blog_app_password=''`; respeta valores ya fijados; idempotente.
- [x] 1.9 Ejecutar `cargo test` y confirmar que los tests nuevos fallan por ausencia de implementación y que el resto sigue verde.

## 2. GREEN — Implementación mínima

- [x] 2.1 `src/services/wordpress.rs`: `resolve_config(pool)` async que lee de `settings` (sin fallback de entorno); mantener `Debug` redactado y la construcción del `Authorization` en el momento de la petición.
- [x] 2.2 `src/tools/blog_comments.rs`: `resolve_config(&self)` async usando `self.db`; conservar el `config` inyectado para tests.
- [x] 2.3 `src/db/repos/settings.rs`: añadir `blog_app_password` a `SENSITIVE_KEYS`.
- [x] 2.4 Migración nueva `migrations/<timestamp>_blog_credentials.sql`: aditiva e idempotente, siembra los tres ajustes.
- [x] 2.5 Nuevos `src/handlers/blog.rs` y `src/routes/blog.rs`: `status`, `password` y `check` (moldes `zennotes`/`opentodo`); registrar en `src/routes/mod.rs` y `src/lib.rs`.
- [x] 2.6 Ejecutar `cargo test` y `cargo check` y confirmar todo en verde.
- [x] 2.7 Frontend: `BlogSection.tsx` (URL base, usuario, contraseña que arranca vacía, «Probar conexión»), `IntegrationsTab.tsx` (sub-pestaña), hooks/API/tipos; `vitest run` verde.

## 3. REFACTOR — Limpieza

- [x] 3.1 `cargo fmt` y `cargo clippy --all-targets -- -D warnings` sin warnings.
- [x] 3.2 `npm run lint` y `npx tsc --noEmit` en `frontend/` sin errores.
- [x] 3.3 Retirar `BLOG_*` de `.env.example`, `.env.j2`, `.env.prod.sample` y actualizar los tests que los usaban.
- [x] 3.4 Re-ejecutar `cargo test` y `vitest run`.

## 4. Verificación E2E y cierre

- [ ] 4.1 Release + despliegue en `co1` (Dockge); `GET /api/health` con la versión nueva y migración aplicada.
- [ ] 4.2 E2E real: configurar usuario + Application Password en Ajustes → Integraciones → Blog y publicar una respuesta a un comentario real; `check` → `{ok:true}`.
- [ ] 4.3 Crear `docs/wordpress-integration.md` y archivar el change con `openspec archive wordpress-credentials-settings`.
