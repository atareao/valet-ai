# Tasks — widget-projection-formats

## 1. Planificación (SDD)

- [x] 1.1 Crear el change `openspec/changes/widget-projection-formats/`.
- [x] 1.2 Escribir `proposal.md` (why / what changes / impact) con la evidencia del falso negativo del 2026-11-02.
- [x] 1.3 Escribir `design.md` (D1–D4, alternativas descartadas y riesgos).
- [x] 1.4 Escribir el delta `specs/orchestrator/agent/spec.md` (1 requisito MODIFICADO).
- [x] 1.5 Validar con `openspec validate widget-projection-formats --strict`.
- [x] 1.6 **Aprobación explícita del usuario** antes de escribir tests/código.

## 2. RED — tests que fallan

- [x] 2.1 Test determinista con el `content` LITERAL capturado en el turno real (lista numerada con énfasis y línea en blanco antes de «¿Cuál prefiere?») → `detect` devuelve 1 `Options` con 3 opciones.
- [x] 2.2 Test: la lista con viñetas separada por línea en blanco de la interrogativa también se detecta.
- [x] 2.3 Test: el `span` NO incluye la línea en blanco ni la interrogativa.
- [x] 2.4 Test: los marcadores de énfasis (`**…**`, `*…*`, `_…_`, `` `…` ``) desaparecen del texto de la opción/ítem, y el `span` no cambia.
- [x] 2.5 Test: tabla de opciones con etiquetas consecutivas → 1 `Options`.
- [x] 2.6 Test: tabla con cabecera `Opción` → 1 `Options`.
- [x] 2.7 Test: tabla de checklist NO se degrada a `Options` (precedencia).
- [x] 2.8 Ejecutar `cargo test` y confirmar rojo en los nuevos, verde en los heredados.
- [x] 2.9 Tests añadidos tras la auditoría: tabla de opciones dentro de un fence (se ignora), guard anti-sufijo, tabla con celdas vacías/separador intercalado, énfasis que no destruye `snake_case`/`aarch64_linux`, y tabla con cabecera genérica (`| Letra | Texto |`).

## 3. GREEN — implementación mínima

- [x] 3.1 `question_options_block`: saltar líneas en blanco antes de comprobar la interrogativa; `span` solo de la lista.
- [x] 3.2 Helper de recorte de énfasis aplicado donde se extraen etiquetas/opciones/ítems.
- [x] 3.3 Detector `options_table_block` con precedencia tras `table_block`.
- [x] 3.4 `cargo test` 100 % verde y `cargo clippy -- -D warnings`.

## 4. REFACTOR

- [x] 4.1 Limpieza, `cargo fmt --check`, `just check-all`.
- [x] 4.2 Correcciones de auditoría (`@rust-reviewer`, veredicto APTO): `strip_markdown_emphasis` empareja solo delimitadores equilibrados (protege `snake_case`); la rama de etiquetas de la tabla descarta la fila de cabecera; helper `table_span`. Suite Rust: 2031 passed / 0 failed.

## 5. Cierre

- [x] 5.1 Verificación desplegada 10.4/10.5 **ejecutada el 2026-11-03** sobre el release **v0.27.0** (contenedor aislado con copia consistente de la BD de producción): un turno real cuya respuesta presenta una lista de opciones acabó con 1 widget **proyectado** (`QuickForm`) con `span:[941,1105]`, el contenido persistido **íntegro** y las opciones **sin** los marcadores de énfasis. La colocación en la UI queda cubierta por los tests deterministas del compositor por tramos + la presencia del `span`.
- [x] 5.2 Archivar el change (`openspec archive widget-projection-formats`).
