pub mod can;
pub mod counter;
pub mod csv;
pub mod minpath;
pub mod plot;
pub mod stats;
pub mod stdio;

#[cfg(test)]
#[ctor::ctor]
fn setup_test_logging() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_test_writer()
        .with_ansi(true)
        .init();
}
