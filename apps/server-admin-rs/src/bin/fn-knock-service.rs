#[cfg(windows)]
fn main() -> anyhow::Result<()> {
    server_admin_rs::windows_service::command_main()
}

#[cfg(not(windows))]
fn main() {
    eprintln!("fn-knock-service is only available on Windows");
    std::process::exit(1);
}
