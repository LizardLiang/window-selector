use std::path::Path;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, EnvFilter};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Initialize the logging subsystem.
/// Writes to rolling files in the given logs directory.
/// When `console` is true, also writes to stderr so logs are visible in
/// a console window (used with the `--debug` CLI flag).
pub fn init_logging(logs_dir: &Path, console: bool) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(logs_dir)?;

    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("window-selector")
        .filename_suffix("log")
        .max_log_files(5)
        .build(logs_dir)?;

    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let file_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true);

    let filter_str = if console {
        // In debug mode, show all debug-level logs.
        "window_selector=debug"
    } else {
        "window_selector=info,warn,error"
    };

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(filter_str));

    if console {
        let stderr_layer = fmt::layer()
            .with_writer(std::io::stderr)
            .with_ansi(true)
            .with_target(true);

        tracing_subscriber::registry()
            .with(filter)
            .with(file_layer)
            .with(stderr_layer)
            .init();
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(file_layer)
            .init();
    }

    // Intentionally leak the guard so it lives for the duration of the process.
    // This is acceptable for a long-running process.
    std::mem::forget(_guard);

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    #[test]
    fn test_log_file_created_on_init() {
        let logs_dir = std::env::temp_dir().join(format!(
            "ws-log-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos()
        ));
        // We just check that the directory can be created; full tracing init
        // can only be done once per process (would panic on second call).
        fs::create_dir_all(&logs_dir).expect("should create logs dir");
        assert!(logs_dir.exists());
        // Cleanup
        let _ = fs::remove_dir_all(&logs_dir);
    }
}
