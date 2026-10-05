use std::future::IntoFuture;
use std::time::Duration;
use tracing_subscriber::EnvFilter;
use valet::config::Config;
use valet::{app_with_state, AppState};

/// Maximum time the HTTP layer is given to drain **after a shutdown signal**.
///
/// This budget is measured from the moment the signal arrives, never from
/// process start. It must not bound normal operation: wrapping the whole
/// `axum::serve(...)` future in this `timeout` makes the clock run while the
/// server is serving happily, so the timeout fires after 5 seconds with no
/// signal in sight and tears down a healthy server. The serve loop is therefore
/// raced against an explicit "drain started" notification and only this phase
/// is timed (see the `select!` in `main`).
///
/// `with_graceful_shutdown` waits for *every* open connection, and this server
/// exposes long-lived SSE streams whose sockets stay open for as long as the
/// client wants. Without a bound, a single streaming client would keep
/// `axum::serve(...)` pending forever and nothing below would ever run, so the
/// runtime would exhaust its grace period and escalate to `SIGKILL`. Keeping
/// this below the 10-second grace period of `podman stop` / `docker stop`
/// guarantees the orderly path completes before the runtime gives up.
const SHUTDOWN_DRAIN_TIMEOUT: Duration = Duration::from_secs(5);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration from environment variables
    let config = Config::from_env();

    // Initialize tracing/logging with level from config
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.log_level)),
        )
        .init();

    tracing::info!(model = %config.openrouter_model, "Configuration loaded");

    // Build full application state (database, orchestrator, tools, guardrails, auth)
    let state = AppState::new_with_orchestrator(&config).await?;

    // Capture the handles we need for an orderly shutdown *before* `state`
    // is moved into the router. Both are cheap clones: the pool is an
    // `Arc`-backed handle and the sender is a broadcast channel.
    let pool = state.db.clone();
    let shutdown_tx = state.shutdown_tx.clone();

    // Spawn the stats cleanup worker (runs hourly, purges old llm_requests)
    let cleanup_handle = tokio::spawn(valet::workers::stats_cleanup::run_cleanup_worker(
        pool.clone(),
    ));

    // Build the application router
    let router = app_with_state(state);

    // Bind and serve on configured host:port.
    //
    // Log the address the socket actually bound to, not `config.port`: with
    // `PORT=0` the OS picks an ephemeral port and `config.port` stays `0`, so
    // logging the configured value would hide the real port. Tests (and
    // operators) rely on this line to discover where the server is listening.
    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let local_addr = listener.local_addr()?;

    tracing::info!("Valet server listening on {local_addr}");

    // Serve until a shutdown signal arrives, then let axum stop accepting
    // new connections and drain the in-flight ones.
    //
    // Why this is not a simple `timeout(SHUTDOWN_DRAIN_TIMEOUT, serve(...))`:
    // `with_graceful_shutdown` receives a future that only resolves when a
    // signal arrives. Wrapping the whole `serve` future in the timeout therefore
    // starts the clock during *normal service*; with no signal it expires after
    // 5 seconds, drops a perfectly healthy server, and `main` falls through to
    // the shutdown path. That was the bug: the budget must bound the drain only,
    // measured from the signal.
    let (drain_started_tx, drain_started_rx) = tokio::sync::oneshot::channel::<()>();

    // The shutdown future resolves as soon as a signal arrives, which is what
    // makes axum start draining. Notify the outer scope at that exact moment so
    // the drain budget is measured from the signal, not from process start.
    let shutdown = async move {
        shutdown_signal().await;
        let _ = drain_started_tx.send(());
    };

    // `axum::serve(...).with_graceful_shutdown(...)` yields a `WithGracefulShutdown`,
    // which implements `IntoFuture` rather than `Future`, so turn it into a real
    // future before pinning it for the `select!` below.
    let server = axum::serve(listener, router)
        .with_graceful_shutdown(shutdown)
        .into_future();
    tokio::pin!(server);

    // `main` returns `Box<dyn Error>` and `select!` arms cannot use `?` on the
    // `std::io::Error` produced by `serve`, so the outcome is stored here and
    // propagated right after the block.
    let serve_result: Result<(), std::io::Error> = tokio::select! {
        result = &mut server => {
            // The server stopped before any shutdown signal (e.g. a bind/accept
            // error); propagate that I/O error.
            result
        }
        // `Ok(()) = ...` disables this branch if the channel closes without a
        // value, so a dropped sender cannot be mistaken for a shutdown signal.
        Ok(()) = drain_started_rx => {
            // The signal arrived: now — and only now — start the drain budget.
            match tokio::time::timeout(SHUTDOWN_DRAIN_TIMEOUT, &mut server).await {
                // Drained cleanly within the budget: fall through to shutdown.
                Ok(result) => result,
                // The drain exceeded its budget. Log loudly and continue
                // shutting down instead of hanging until the runtime escalates
                // to SIGKILL. This can only happen after a signal.
                Err(_elapsed) => {
                    tracing::warn!(
                        timeout_secs = SHUTDOWN_DRAIN_TIMEOUT.as_secs(),
                        "Graceful drain timed out; connections still open, forcing shutdown"
                    );
                    Ok(())
                }
            }
        }
    };
    serve_result?;

    tracing::info!("Shutdown signal received, stopping workers and closing the database pool");

    // Notify every worker subscribed to the shared shutdown channel. A send
    // error only means there is no active subscriber, so it is safe to ignore.
    if let Some(tx) = shutdown_tx {
        let _ = tx.send(());
    }

    // The cleanup worker loops forever, so abort it explicitly instead of
    // waiting for it to observe the shutdown channel. `abort()` only marks the
    // task, so we await the handle to guarantee the task is really gone before
    // closing the pool; otherwise "abort, then close" would just be a promise.
    cleanup_handle.abort();
    // A cancelled task surfaces as a `JoinError` with `is_cancelled() == true`;
    // that is the expected outcome here, so it is deliberately discarded.
    let _ = cleanup_handle.await;

    // Close the pool so requests that were already drained can finish their
    // in-flight queries. This is best-effort for the background workers: their
    // `JoinHandle`s are owned inside `AppState::new_with_orchestrator` and are
    // never awaited, so a worker caught mid-LLM-call may be lost when the
    // process exits. Shutdown is orderly for the HTTP layer and the pool, but
    // not for in-flight worker work.
    pool.close().await;

    tracing::info!("Valet server shut down cleanly");

    Ok(())
}

/// Resolve when the process receives either `SIGINT` (Ctrl+C) or `SIGTERM`.
///
/// `valet` normally runs as PID 1 inside its container. The kernel does **not**
/// apply the default action for signals to PID 1, so a plain `SIGTERM` sent by
/// `podman stop` / `docker stop` would otherwise be ignored and the runtime
/// would escalate to `SIGKILL` after its 10-second grace period. Installing
/// handlers for both signals lets the container stop promptly and cleanly.
async fn shutdown_signal() {
    let ctrl_c = async {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                tracing::info!("Received SIGINT (Ctrl+C), initiating graceful shutdown");
            }
            Err(error) => {
                tracing::error!(%error, "Failed to install SIGINT/Ctrl+C handler");
                // Never resolve: fall back to the other signal branch.
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut stream) => {
                stream.recv().await;
                tracing::info!("Received SIGTERM, initiating graceful shutdown");
            }
            Err(error) => {
                tracing::error!(%error, "Failed to install SIGTERM handler");
                std::future::pending::<()>().await;
            }
        }
    };

    // On non-Unix platforms there is no SIGTERM; only Ctrl+C can stop the server.
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
}

#[cfg(test)]
mod tests {
    use serial_test::serial;
    use std::env;
    use valet::AppState;

    /// Verify that the production initialisation path
    /// (`AppState::new_with_orchestrator`) correctly wires up every
    /// optional component (orchestrator, tool_registry, guardrails,
    /// auth_config) to `Some` instead of leaving them as `None`.
    ///
    /// **Red-phase rationale:**
    ///
    /// The current `main()` manually constructs `AppState` with every
    /// optional field hard-coded to `None`. It does **not** call
    /// `Config::from_env()` or `AppState::new_with_orchestrator()`.
    ///
    /// This test encodes the *desired* contract: when the production
    /// entry point is used, all subsystems must be wired up. Once
    /// `main()` is refactored to use `Config::from_env()` +
    /// `AppState::new_with_orchestrator()`, this assertion will hold.
    #[tokio::test(flavor = "multi_thread")]
    #[serial]
    async fn test_production_state_initialization() {
        // -----------------------------------------------------------------
        // Arrange: set the minimum environment variables required by
        //          `AppState::new_with_orchestrator`.
        // -----------------------------------------------------------------
        env::set_var("OPENROUTER_API_KEY", "sk-test-key-for-unit-test");
        env::set_var("AUTH_ENABLED", "true");
        // AUTH_ISSUER_URL and AUTH_REDIRECT_URL have no default anymore
        // (fail-closed), so they must be set explicitly for a complete, valid
        // auth configuration.
        env::set_var("AUTH_ISSUER_URL", "https://issuer.example");
        env::set_var("AUTH_CLIENT_ID", "test-client");
        env::set_var("AUTH_CLIENT_SECRET", "test-secret");
        env::set_var(
            "AUTH_REDIRECT_URL",
            "http://localhost:3000/api/auth/callback",
        );
        env::set_var("JWT_SECRET", "test-jwt-secret");

        let tmp_dir = env::temp_dir();
        let db_filename = "valet_test_production_state.db";
        let db_path = tmp_dir.join(db_filename);
        // Remove any stale database from a previous (failed) run
        let _ = std::fs::remove_file(&db_path);

        // -----------------------------------------------------------------
        // Act: use the SAME entry point that main() SHOULD call
        // -----------------------------------------------------------------
        use valet::config::Config;

        // Create a Config with test values
        let mut test_config = Config::from_env();
        test_config.database_url = db_path
            .to_str()
            .expect("temp path must be valid UTF-8")
            .to_string();
        // Override env vars are already set above (OPENROUTER_API_KEY, AUTH_*, etc.)
        let state = AppState::new_with_orchestrator(&test_config)
            .await
            .expect("new_with_orchestrator should succeed with minimal env vars");

        // -----------------------------------------------------------------
        // Assert: every component is wired up
        // -----------------------------------------------------------------
        assert!(
            state.orchestrator.is_some(),
            "orchestrator is None but should be Some — \
             main() currently hard-codes it to None"
        );
        assert!(
            state.tool_registry.is_some(),
            "tool_registry is None but should be Some — \
             main() currently hard-codes it to None"
        );
        assert!(
            state.guardrails.is_some(),
            "guardrails is None but should be Some — \
             main() currently hard-codes it to None"
        );
        assert!(
            state.auth_config.is_some(),
            "auth_config is None but should be Some — \
             main() currently hard-codes it to None"
        );

        // Additional verification on auth config values
        let auth = state.auth_config.as_ref().unwrap();
        assert!(auth.enabled, "auth should be enabled per AUTH_ENABLED=true");
        assert_eq!(auth.client_id, "test-client");

        // -----------------------------------------------------------------
        // Teardown: remove the temporary database + WAL/SHM artifacts
        // -----------------------------------------------------------------
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(tmp_dir.join("valet_test_production_state.db-wal"));
        let _ = std::fs::remove_file(tmp_dir.join("valet_test_production_state.db-shm"));

        // Clean up environment variables so they don't leak into other tests
        env::remove_var("OPENROUTER_API_KEY");
        env::remove_var("AUTH_ENABLED");
        env::remove_var("AUTH_ISSUER_URL");
        env::remove_var("AUTH_CLIENT_ID");
        env::remove_var("AUTH_CLIENT_SECRET");
        env::remove_var("AUTH_REDIRECT_URL");
        env::remove_var("JWT_SECRET");
    }
}
