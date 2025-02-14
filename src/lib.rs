pub mod can;
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
