# Tasks

## 1. RED — tests que fallan

- [x] 1.1 Test: las `criteria_true` del catálogo de `agenda`, `pendientes`, `entorno` y `widgets` declaran el **saludo de apertura del día**.
- [x] 1.2 Test: las `criteria_true` de `recuerdos` y `web` **no** lo declaran.

## 2. GREEN — implementación

- [x] 2.1 Actualizar las cuatro `criteria_true` del catálogo con el saludo de apertura, conservando el resto del texto.
- [x] 2.2 Reetiquetar los tests de integridad del catálogo si alguno fijaba el texto anterior.

## 3. REFACTOR

- [x] 3.1 `cargo fmt` y `cargo clippy --all-targets -- -D warnings` sin warnings.

## 4. Verificación

- [x] 4.1 `just check-all` en verde.
- [x] 4.2 Arnés con el enrutador forzado y `--repeat 3`, umbrales **intactos**: el briefing matutino queda cubierto y la cobertura sube a ~100 %.
- [x] 4.3 Sin sobreactivación: las activaciones por skill no se disparan y el ahorro se mantiene ≥20 %.

## 5. Cierre

- [x] 5.1 Review con `@rust-reviewer` (solo lectura) y hallazgos aplicados.
- [x] 5.2 PR a `development` y `openspec archive morning-briefing-criteria`.
