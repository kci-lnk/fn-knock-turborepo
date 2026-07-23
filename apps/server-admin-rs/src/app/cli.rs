use std::env;

use anyhow::Context;
use serde_json::json;

use crate::{
    i18n::{DEFAULT_LOCALE, Translator},
    settings::Settings,
    storage::legacy_redis_migration::{self, LegacyRedisMigrationOptions},
    store::Store,
};

pub(super) fn print_help() {
    println!("server-admin-rs");
    println!();
    println!("Commands:");
    println!("  reset-panel-password    Clear admin panel password/session state");
    println!(
        "  migrate-redis-to-sqlite Import legacy Redis fn_knock:* data into SQLite, then delete source keys"
    );
}

pub(super) async fn reset_panel_password_command() -> anyhow::Result<()> {
    let args = env::args().skip(2).collect::<Vec<_>>();
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "-h" | "--help"))
    {
        let locale = env::var("FN_KNOCK_LOCALE").unwrap_or_else(|_| DEFAULT_LOCALE.to_string());
        let translator = Translator::new(locale);
        println!("{}", translator.t("server.dockerAdminPanel.resetHelp"));
        return Ok(());
    }
    if let Some(arg) = args.first() {
        anyhow::bail!("unknown argument for reset-panel-password: {arg}");
    }

    let settings = Settings::from_env();
    let store = Store::connect(&settings.sqlite_path)
        .await
        .context("open SQLite storage for admin panel password reset")?;
    let locale = store
        .locale()
        .await
        .ok()
        .and_then(|value| {
            value
                .get("default_locale")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| DEFAULT_LOCALE.to_string());
    let translator = Translator::new(locale);

    let summary = store.reset_docker_admin_password_state().await?;
    println!("{}", translator.t("server.dockerAdminPanel.resetCleared"));
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "passwordCleared": summary.password_cleared,
            "sessionsCleared": summary.sessions_cleared,
            "loginFailuresCleared": summary.login_failures_cleared,
        }))?
    );
    println!("{}", translator.t("server.dockerAdminPanel.resetNextVisit"));
    Ok(())
}

pub(super) async fn migrate_redis_to_sqlite_command() -> anyhow::Result<()> {
    let args = env::args().skip(2).collect::<Vec<_>>();
    let force = args.iter().any(|arg| arg == "--force");
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "-h" | "--help"))
    {
        println!("Usage: server-admin-rs migrate-redis-to-sqlite [--force]");
        println!();
        println!("Imports legacy Redis fn_knock:* data into the configured SQLite database.");
        println!(
            "By default it will not overwrite an SQLite database that already has fn_knock:* keys."
        );
        println!("After a successful import it deletes legacy fn_knock:* keys from source Redis.");
        println!("Use --force to clear the SQLite fn_knock:* keyspace before importing.");
        return Ok(());
    }
    if let Some(arg) = args.iter().find(|arg| arg.as_str() != "--force") {
        anyhow::bail!("unknown argument for migrate-redis-to-sqlite: {arg}");
    }

    let settings = Settings::from_env();
    if !legacy_redis_migration::migration_allowed_for_runtime_target(&settings.runtime_target) {
        anyhow::bail!("legacy Redis migration is unavailable for fpk-lite");
    }
    let store = Store::connect(&settings.sqlite_path)
        .await
        .context("open SQLite storage for legacy Redis migration")?;
    let outcome = legacy_redis_migration::migrate_if_available(
        &store,
        &settings.legacy_redis_url,
        LegacyRedisMigrationOptions {
            require_source: true,
            force,
            cleanup_source: true,
        },
    )
    .await?;
    println!("{}", outcome.summary());
    Ok(())
}
