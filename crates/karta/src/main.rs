use kovi_onebot::{OneBotDriver, load_local_conf};
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load the OneBot driver config from the [server] table of kovi.conf.toml
    // (prompts interactively on first run).
    let driver_config = load_local_conf()?;
    let driver = OneBotDriver::new(driver_config);
    let bot = kovi::build_bot!(driver; karta);
    bot.run().await;
    Ok(())
}
