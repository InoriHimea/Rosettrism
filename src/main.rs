#[tokio::main]
async fn main() -> anyhow::Result<()> {
    rosettrism::cli::run().await
}
