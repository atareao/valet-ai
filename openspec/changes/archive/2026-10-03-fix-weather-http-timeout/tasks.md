# Tasks: fix-weather-http-timeout

## Bloque 0 — Baseline (antes de tocar nada)
- [x] 0.1 Registrar el baseline por CLI: `cargo test --no-fail-fast` (passed/failed), `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`.
- [x] 0.2 Inventario: `grep -n "reqwest::Client" src/tools/weather.rs`; confirmar que `WeatherTool::new` usa `Client::new()` (sin timeout) frente a `ReverseGeocodeTool` (`src/tools/geo.rs`), que ya fija 30 s.

## Bloque 1 — RED
- [x] 1.1 Seam no conductual: `WeatherTool` gana `base_url: String` (default `https://api.openweathermap.org/data/2.5`) y `timeout: Option<Duration>`; `get_current_weather`/`get_forecast` construyen la URL con `format!("{}/weather...", self.base_url)`; helper `build_client(timeout)` que aplica `.timeout(d)` si `Some`; getter `#[cfg(test)] fn http_timeout(&self)`; constructor `#[cfg(test)] fn with_base_url_and_timeout(db, api_key, base_url, timeout)`. `new` mantiene el comportamiento ACTUAL: `timeout = None` (sin timeout).
- [x] 1.2 Test RED: `WeatherTool::new(pool, key).http_timeout()` DEBE ser `Some(Duration::from_secs(30))`. Hoy `new` deja `None` → rojo por aserción.
- [x] 1.3 Test guarda (wiremock, pasa ya): con `with_base_url_and_timeout(..., Some(50 ms))` y un mock `/weather` con `set_delay(500 ms)`, `execute` devuelve `Err(ToolError::ExecutionError)`. Confirma que el mecanismo de timeout funciona cuando se fija.
- [x] 1.4 Verificar por CLI: 1.2 en rojo por aserción y el resto en verde.

## Bloque 2 — GREEN
- [x] 2.1 `src/tools/weather.rs`: `WeatherTool::new` fija `timeout = Some(Duration::from_secs(30))`.
- [x] 2.2 Tests GREEN: 1.2 y 1.3 pasan.
- [x] 2.3 Verificar por CLI: `cargo test --no-fail-fast` (0 failed), `cargo check`.

## Bloque 3 — REFACTOR y cierre
- [x] 3.1 `cargo fmt --check` y `cargo clippy --all-targets -- -D warnings` limpios.
- [x] 3.2 Revisión con `rust-reviewer`.
- [x] 3.3 PR a `development`; tras el merge, PR de archivado (`openspec archive fix-weather-http-timeout`).
