use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let _devices = nusb::list_devices().await?;
    Ok(())
}
