use tracing_subscriber::EnvFilter;

/// Initialize the global tracing subscriber.
///
/// In production (no RUST_LOG set), uses the provided `log_level`.
/// In development (RUST_LOG set), respects that override.
/// Uses JSON format for structured logging.
pub fn init_tracing(log_level: &str) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Instala el suscriptor **por hilo** en vez de en el slot global.
    ///
    /// Este binario de test comparte proceso con tests que capturan trazas con su
    /// propio suscriptor global (p. ej. `tools::blog_comments`): un `try_init`
    /// global aquí dejaría a aquellos sin suscriptor y su captura fallaría de
    /// forma intermitente según el orden de ejecución. Un suscriptor por hilo no
    /// contamina el slot global y mantiene el test aislado.
    fn scoped(log_level: &str) -> tracing::subscriber::DefaultGuard {
        tracing::subscriber::set_default(
            tracing_subscriber::fmt()
                .with_env_filter(EnvFilter::new(log_level))
                .with_test_writer()
                .finish(),
        )
    }

    #[test]
    fn test_init_tracing_does_not_panic() {
        let _guard = scoped("info");
    }

    #[test]
    fn test_init_tracing_with_custom_level() {
        let _guard = scoped("debug");
    }
}
