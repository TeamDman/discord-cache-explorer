use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::Directive;
use tracing_subscriber::prelude::*;
use tracing_subscriber::util::SubscriberInitExt;

/// # Errors
///
/// Returns an error if the default tracing directive is invalid.
pub fn init_tracing(debug: bool) -> eyre::Result<()> {
    let level = if debug {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };
    let directive: Directive = level.into();
    let env_filter = EnvFilter::builder()
        .with_default_directive(directive)
        .from_env_lossy();

    if let Err(error) = tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().pretty().without_time())
        .try_init()
    {
        eprintln!("Failed to initialize tracing subscriber: {error}");
    }

    Ok(())
}
