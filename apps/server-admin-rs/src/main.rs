#[tokio::main]
async fn main() -> anyhow::Result<()> {
    server_admin_rs::app::run().await
}
