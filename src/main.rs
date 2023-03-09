fn main() -> anyhow::Result<()> {
    setup_logging()?;

    log::info!("Hello World");

    Ok(())
}

fn setup_logging() -> anyhow::Result<()> {
    let loglevel = log::LevelFilter::Debug;

    fern::Dispatch::new()
        .format(|out, message, record| out.finish(format_args!("[{}] {}", record.level(), message)))
        .level(loglevel)
        .chain(std::io::stdout())
        .apply()?;

    Ok(())
}
