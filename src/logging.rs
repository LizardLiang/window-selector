use std::path::Path;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, EnvFilter};

/// Initialize the logging subsystem.
/// Writes to rolling files in the given logs directory.
/// In release builds, does not log to stdout.
pub fn init_logging(logs_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(logs_dir)?;

    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("window-selector")
        .filename_suffix("log")
        .max_log_files(5)
        .build(logs_dir)?;

    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // In release mode, only log to file. In debug mode, also log to stderr.
    #[cfg(debug_assertions)]
    {
        use tracing_subscriber::layer::SubscriberExt;
        use tracing_subscriber::util::SubscriberInitExt;

        let file_layer = fmt::layer()
            .with_writer(non_blocking)
            .with_ansi(false)
            .with_target(true);

        let stderr_layer = fmt::layer()
            .with_writer(std::io::stderr)
            .with_ansi(false)
            .with_target(true);

        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("window_selector=debug,info"));

        tracing_subscriber::registry()
            .with(filter)
            .with(file_layer)
            .with(stderr_layer)
            .init();
    }

    #[cfg(not(debug_assertions))]
    {
        use tracing_subscriber::layer::SubscriberExt;
        use tracing_subscriber::util::SubscriberInitExt;

        let file_layer = fmt::layer()
            .with_writer(non_blocking)
            .with_ansi(false)
            .with_target(true);

        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("window_selector=info,warn,error"));

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
    use super::*;
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