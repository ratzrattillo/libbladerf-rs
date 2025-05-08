use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let devices = nusb::list_devices().await?;
    Ok(())
}
