use eyre::Result;

#[tokio::main]
async fn main() -> Result<()> {
    ssh_clipboard::cli::run().await
}
