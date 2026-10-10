# Tasks

## 0. Preparación y aprobación

- [x] 0.1 Presentar este change (proposal + specs + tasks) y **esperar aprobación explícita** antes de escribir código.

## 1. RED: fijar el contrato

- [x] 1.1 Tests que fallan (catálogo): el catálogo declara **nueve** skills e incluye `email` con sus cuatro herramientas; las claves de fragmento son nueve e incluyen `SKILL_EMAIL_PROMPT`; el core más las nueve skills cubren **veinticuatro** herramientas sin huérfanas; `email` declara umbral `0.10`.
- [x] 1.2 Tests que fallan (router): el fallo abierto con todas las skills habilitadas expone las **veinticuatro** herramientas.
- [x] 1.3 Tests que fallan (`tools/email`, sin red): nombre, descripción en español y esquema de parámetros de las cuatro herramientas; `email_mark_read` declara `NoConfirm` y **`email_send` declara `ExplicitApproval`**; `email_get_body`, `email_mark_read` y `email_send` con `reply_to_uid` exigen `uid`; sin API key configurada las cuatro devuelven un error accionable **sin salir a la red**.
- [x] 1.4 Tests que fallan (`tools/email`, sin red): `email_send` exige `text` y, sin `reply_to_uid`, también `to` y `subject`; un argumento inválido devuelve `InvalidArguments` sin salir a la red.
- [x] 1.5 Tests que fallan (`services/apimail` con `wiremock`): la URL base y la clave salen de `settings` y caen al entorno; el listado pide `mailbox=INBOX&unseen=true` y aplana remitente, asunto, fecha y `uid`; el cuerpo pide `/{uid}/body?mailbox=INBOX`; marcar leído hace `PATCH` con `{"add":["\\Seen"]}`; **enviar hace `POST /api/messages` con `to`, `subject` y `text`, y responder con `reply_to_uid` resuelve el destinatario del envelope y compone `Re: <asunto>` sin duplicar el prefijo**; un `401` se traduce a credenciales inválidas, un `404` de mensaje a «no existe», un `502 smtp_error` a «no se pudo entregar» y un `503` a «servidor de correo no disponible»; la clave viaja como `Authorization: Bearer`.
- [x] 1.6 Tests que fallan (migración): tras migrar existen `ROUTER_SKILL_EMAIL_ENABLED`, `SKILL_EMAIL_PROMPT` y `apimail_base_url` (con `https://apimail.territoriolinux.es`); un valor previo no se sobrescribe; la migración es idempotente.
- [x] 1.7 Tests que fallan (frontend): la pestaña «API Keys» muestra los campos de URL base y API key de apimail y los envía al guardar.

## 2. GREEN

- [x] 2.1 `src/services/apimail.rs` y `src/services/mod.rs`: cliente con base inyectable, resolución de configuración, las cuatro operaciones —incluida la lectura del envelope para responder— y traducción de errores.
- [x] 2.2 `src/tools/email.rs` y `src/tools/mod.rs`: las cuatro herramientas.
- [x] 2.3 `src/lib.rs`: registrar las cuatro herramientas en `build_tool_registry`.
- [x] 2.4 Las variables de entorno `APIMAIL_BASE_URL` y `APIMAIL_API_KEY` se leen **directamente en `src/services/apimail.rs`** (mismo precedente que Strava en `src/services/strava.rs`), en cada llamada y antes que el valor por defecto. **No se toca `src/config.rs`**: la resolución por llamada es la que exige la spec («las claves SHALL leerse en cada llamada»), y `Config::from_env()` se lee una sola vez al arrancar.
- [x] 2.5 `src/orchestrator/skills.rs`: variante `Email`, entrada en el catálogo y actualización de los tests de integridad.
- [x] 2.6 Migración `20261010000006_email_skill.sql`: siembra idempotente de las cuatro claves, con su target de test `email-migration`.
- [x] 2.7 `frontend/src/components/SettingsDialog.tsx`: los dos campos de la pestaña «API Keys».
- [x] 2.8 `.env.example` y `.env.j2`: documentar `APIMAIL_BASE_URL` y `APIMAIL_API_KEY`.

## 3. REFACTOR / limpieza

- [x] 3.1 `cargo fmt` + `cargo clippy --all-targets -- -D warnings` (0 warnings) y `npm run lint`.
- [x] 3.2 Grep de listas cerradas hermanas (conteos de herramientas en tests, `ALL_TOOLS`, `tests/api/tools.rs`) para que ningún ocho, veinte o veintitrés se quede atrás.

## 4. VERIFY / cierre

- [x] 4.1 `cargo test --no-fail-fast` (1165 passed / 0 failed), `cargo clippy`, `cargo fmt --check`, `npx vitest run` (349 passed), `npx tsc --noEmit` y `openspec validate email-skill --strict` en verde.
- [x] 4.2 Review de `@rust-reviewer` (cambios necesarios) y `@react-reviewer` (aprobado); hallazgos aplicados: `email_get_body` propaga `null` en vez de `""`, `Debug` de `ApimailConfig` enmascara la clave, `email_send` resuelve la configuración una sola vez, `to` rechaza direcciones en blanco, responder a un mensaje sin asunto no deja un `Re:` colgando, y los caminos de éxito y la traducción de errores de las cuatro herramientas ganan test a nivel de tool.
- [x] 4.3 `openspec archive email-skill`.
- [ ] 4.4 PR a `development` y merge.
- [ ] 4.5 Comprobar en producción con la API key configurada: la skill aparece en «Skills» con su interruptor, umbral, pregunta, criterios y fragmento; el modelo lista no leídos, lee un cuerpo, lo marca como leído y **envía una respuesta que pide aprobación antes de salir**.
