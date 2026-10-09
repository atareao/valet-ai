# Tasks: Estimador determinista de tokens JSON

## Bloque 1 — Estimador (funciones puras)
- [x] 1.1 RED/GREEN: módulo con `estimate_json_tokens` y sus helpers privados (símbolos
  estructurales, cadenas ASCII/no-ASCII, literales, números, escapes, cadena vacía).
- [x] 1.2 RED/GREEN: determinismo y monotonía (más contenido ⇒ estimación ≥).

## Bloque 2 — Integración en la Capa C
- [x] 2.1 RED/GREEN: `payload_token_count` usa `estimate_json_tokens` sobre el JSON minificado.
- [x] 2.2 Eliminar el uso del heurístico de markdown en la Capa C, sin tocar los mensajes de chat.

## Bloque 3 — Calibración y presupuesto
- [x] 3.1 Comparar la estimación con los tokens reales registrados en las stats del proveedor
  sobre una muestra representativa; documentar el procedimiento (sin dependencias).
- [x] 3.2 Ajustar los ratios si la desviación es grande.
- [x] 3.3 Comprobar si el default 500 sigue siendo razonable y ajustarlo si procede.

## Bloque 4 — Verificación y limpieza
- [x] 4.1 `cargo fmt --check` limpio.
- [x] 4.2 `cargo clippy --all-targets -- -D warnings` limpio.
- [x] 4.3 `cargo test` completo en verde, sin regresiones.
- [x] 4.4 `openspec validate json-token-estimator --strict` válido.
