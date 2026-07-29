#[cfg(has_http)]
use std::future::Future;
#[cfg(has_database)]
use std::path::Path;
#[cfg(any(has_http, has_database))]
use std::time::Duration;

use clap::{Parser, Subcommand};

#[cfg(has_http)]
macro_rules! lifecycle_info {
    ($op:literal, $msg:literal) => {
        tracing::info!(feature = "lifecycle", operation = $op, result = "ok", $msg)
    };
}

#[derive(Parser)]
#[command(
    name = "skel_rs",
    version,
    about = "Clean Architecture web service template"
)]
pub struct Cli {
    #[arg(long, global = true, default_value = "config.yaml")]
    pub config: String,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    #[cfg(has_http)]
    #[command(about = "Start the web server")]
    Serve {
        #[arg(long, help = "Validate startup without binding a TCP port")]
        dry_run: bool,
    },
    #[command(about = "Configuration management")]
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
    #[cfg(has_http)]
    #[command(about = "Check if the server is accepting connections (for Docker health check)")]
    HealthCheck,
    #[cfg(has_database)]
    #[command(about = "Database management")]
    Db {
        #[arg(long, help = "Path to migrations directory")]
        migrations_path: Option<String>,

        #[command(subcommand)]
        command: DbCommands,
    },
}

#[derive(Subcommand)]
pub enum ConfigCommands {
    #[command(about = "Validate configuration file")]
    Check,
    #[command(
        about = "Show resolved configuration (after env var substitution, credentials redacted)"
    )]
    Show,
}

#[cfg(has_database)]
#[derive(Subcommand)]
pub enum DbCommands {
    #[command(about = "Run pending database migrations")]
    Migrate,
    #[command(about = "Show migration status")]
    Status,
}

#[allow(
    clippy::unused_async,
    reason = "feature=[] 时 match 分支无 await；has_http/has_database 启用时被消费"
)]
pub async fn dispatch(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        #[cfg(has_http)]
        Commands::Serve { dry_run } => run_serve(&cli.config, dry_run).await,
        #[cfg(has_http)]
        Commands::HealthCheck => run_health_check(&cli.config),
        Commands::Config { command } => match command {
            ConfigCommands::Check => run_config_check(&cli.config),
            ConfigCommands::Show => run_config_show(&cli.config),
        },
        #[cfg(has_database)]
        Commands::Db {
            migrations_path,
            command,
        } => match command {
            DbCommands::Migrate => run_db_migrate(&cli.config, migrations_path.as_deref()).await,
            DbCommands::Status => run_db_status(&cli.config, migrations_path.as_deref()).await,
        },
    }
}

#[cfg(any(has_http, has_database))]
pub fn require_section<T>(opt: Option<T>, section: &str, config_path: &str) -> anyhow::Result<T> {
    opt.ok_or_else(|| anyhow::anyhow!("{section} section not configured in {config_path}"))
}

#[cfg(any(has_http, has_database))]
fn init_validated_telemetry(config: &adapters::AppConfig) {
    crate::telemetry::init_telemetry(&config.telemetry)
        .expect("internal error: load_and_parse_config validated log_level/log_format upstream");
}

#[cfg(has_http)]
#[path = "signal.rs"]
mod signal;

#[cfg(has_http)]
pub async fn run_serve(config_path: &str, dry_run: bool) -> anyhow::Result<()> {
    run_serve_with_shutdown(config_path, dry_run, self::signal::shutdown_signal()).await
}

#[cfg(has_http)]
pub async fn run_serve_with_shutdown<F: Future<Output = ()> + Send + 'static>(
    config_path: &str,
    dry_run: bool,
    shutdown: F,
) -> anyhow::Result<()> {
    run_serve_with_shutdown_inner(config_path, dry_run, Box::pin(shutdown)).await
}

#[cfg(has_http)]
async fn run_serve_with_shutdown_inner(
    config_path: &str,
    dry_run: bool,
    shutdown: std::pin::Pin<Box<dyn Future<Output = ()> + Send>>,
) -> anyhow::Result<()> {
    let config = adapters::load_and_parse_config(config_path)?;
    init_validated_telemetry(&config);

    adapters::log_startup_banner(&config, env!("CARGO_PKG_VERSION"));

    let server_config = require_section(config.server.as_ref(), "server", config_path)?;

    if dry_run {
        lifecycle_info!("serve.dry_run", "dry-run complete, exiting");
        return Ok(());
    }

    let state = adapters::AppState::new();

    #[cfg(has_database)]
    let (state, db_pool) = build_state_with_db(state, config.database.as_ref()).await?;

    #[cfg(has_database)]
    let router = build_serve_router(state, db_pool.as_ref());
    #[cfg(not(has_database))]
    let router = crate::server::build_router(state);
    let drain_timeout = Duration::from_secs(server_config.drain_timeout_secs);
    let serve_result =
        crate::server::start_server(server_config, router, shutdown, drain_timeout).await;

    #[cfg(has_database)]
    if let Some(pool) = db_pool {
        lifecycle_info!("db.pool.close", "closing database pool");
        pool.close().await;
    }

    serve_result.map_err(anyhow::Error::from)
}

#[cfg(all(has_http, feature = "sqlite"))]
fn build_serve_router(
    state: adapters::AppState,
    db_pool: Option<&adapters::database::DbPool>,
) -> axum::Router {
    let Some(pool) = db_pool else {
        lifecycle_info!(
            "examples.skip",
            "no database configured, skipping example routes"
        );
        return crate::server::build_router(state);
    };
    crate::server::build_router_with_examples(state, adapters::SqlExampleStore::new(pool.clone()))
}

#[cfg(all(has_http, has_database, not(feature = "sqlite")))]
fn build_serve_router(
    state: adapters::AppState,
    db_pool: Option<&adapters::database::DbPool>,
) -> axum::Router {
    let _ = db_pool;
    tracing::warn!(
        feature = "lifecycle",
        operation = "examples.unsupported",
        result = "skipped",
        "example routes require the sqlite feature; the configured database backend has no ExampleStore implementation",
    );
    crate::server::build_router(state)
}

#[cfg(all(has_http, has_database))]
async fn build_state_with_db(
    state: adapters::AppState,
    db: Option<&adapters::DatabaseConfig>,
) -> anyhow::Result<(adapters::AppState, Option<adapters::database::DbPool>)> {
    let Some(db_config) = db else {
        lifecycle_info!("db.skip", "no database configured, skipping pool creation");
        return Ok((state, None));
    };
    let pool = crate::database::create_pool(db_config).await?;
    let expected_migrations =
        crate::database::expected_migration_count(Path::new(&db_config.migrations_path)).await?;
    let policy = adapters::DbReadinessPolicy {
        expected_migrations,
        health_check_timeout_secs: db_config.pool.health_check_timeout_secs,
    };
    let state = state.with_db_pool(pool.clone(), policy);
    Ok((state, Some(pool)))
}

#[cfg(has_http)]
pub fn run_health_check(config_path: &str) -> anyhow::Result<()> {
    use std::net::ToSocketAddrs as _;
    let config = adapters::load_and_parse_config(config_path)?;
    init_validated_telemetry(&config);
    let server_config = require_section(config.server.as_ref(), "server", config_path)?;
    let health_check_config = config
        .health_check
        .as_ref()
        .expect("internal error: load_and_parse_config guarantees health_check when server exists");
    let addr = health_check_target(&health_check_config.host, server_config.port);
    let addrs: Vec<_> = addr
        .to_socket_addrs()
        .map_err(|e| anyhow::anyhow!("invalid health check address {addr}: {e}"))?
        .collect();
    let total_timeout = Duration::from_secs(health_check_config.connect_timeout_secs);
    let attempt_timeout = per_address_timeout(total_timeout, addrs.len());
    let started = std::time::Instant::now();
    let mut last_err: Option<std::io::Error> = None;
    for socket_addr in &addrs {
        match std::net::TcpStream::connect_timeout(socket_addr, attempt_timeout) {
            Ok(_) => {
                log_health_check_result(&addr, adapters::elapsed_ms(started), None);
                return Ok(());
            }
            Err(e) => last_err = Some(e),
        }
    }
    log_health_check_result(&addr, adapters::elapsed_ms(started), last_err.as_ref());
    Err(health_check_failure(&addr, last_err))
}

#[cfg(has_http)]
fn log_health_check_result(addr: &str, duration_ms: u64, error: Option<&std::io::Error>) {
    match error {
        None => tracing::debug!(
            feature = "health",
            operation = "health_check.connect",
            result = "ok",
            duration_ms,
            addr,
            "health check connected",
        ),
        Some(e) => tracing::warn!(
            feature = "health",
            operation = "health_check.connect",
            result = "error",
            duration_ms,
            addr,
            error = %e,
            "health check cannot connect",
        ),
    }
}

#[cfg(has_http)]
fn health_check_target(host: &str, port: u16) -> String {
    if host.contains(':') && !host.starts_with('[') {
        return format!("[{host}]:{port}");
    }
    format!("{host}:{port}")
}

#[cfg(has_http)]
fn per_address_timeout(total: Duration, address_count: usize) -> Duration {
    let divisor = u32::try_from(address_count).unwrap_or(u32::MAX).max(1);
    total / divisor
}

#[cfg(has_http)]
fn health_check_failure(addr: &str, last_err: Option<std::io::Error>) -> anyhow::Error {
    let Some(err) = last_err else {
        return anyhow::anyhow!("health check failed: {addr} resolved to no socket address");
    };
    anyhow::anyhow!("health check failed: {err}")
}

#[expect(clippy::print_stdout, reason = "CLI command output")]
pub fn run_config_check(config_path: &str) -> anyhow::Result<()> {
    let msg = adapters::check_config(config_path)?;
    println!("{msg}");
    Ok(())
}

#[expect(clippy::print_stdout, reason = "CLI command output")]
pub fn run_config_show(config_path: &str) -> anyhow::Result<()> {
    let yaml = adapters::show_config(config_path)?;
    print!("{yaml}");
    Ok(())
}

#[cfg(has_database)]
type DbTaskFuture = std::pin::Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>>;

#[cfg(has_database)]
pub async fn with_db_pool<F, Fut>(
    config_path: &str,
    migrations_path_override: Option<&str>,
    f: F,
) -> anyhow::Result<()>
where
    F: FnOnce(adapters::database::DbPool, std::path::PathBuf, Duration) -> Fut + Send + 'static,
    Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
{
    with_db_pool_inner(
        config_path,
        migrations_path_override,
        Box::new(move |pool, path, timeout| Box::pin(f(pool, path, timeout))),
    )
    .await
}

#[cfg(has_database)]
async fn with_db_pool_inner(
    config_path: &str,
    migrations_path_override: Option<&str>,
    f: Box<
        dyn FnOnce(adapters::database::DbPool, std::path::PathBuf, Duration) -> DbTaskFuture + Send,
    >,
) -> anyhow::Result<()> {
    let config = adapters::load_and_parse_config(config_path)?;
    init_validated_telemetry(&config);
    let db_config = require_section(config.database.as_ref(), "database", config_path)?;

    let migrations_path = migrations_path_override.unwrap_or(&db_config.migrations_path);
    let health_check_timeout = Duration::from_secs(db_config.pool.health_check_timeout_secs);

    let pool = crate::database::create_pool(db_config).await?;
    let path_buf = Path::new(migrations_path).to_path_buf();
    let result = f(pool.clone(), path_buf, health_check_timeout).await;
    pool.close().await;
    result
}

#[cfg(has_database)]
#[expect(clippy::print_stdout, reason = "CLI command output")]
pub async fn run_db_migrate(
    config_path: &str,
    migrations_path_override: Option<&str>,
) -> anyhow::Result<()> {
    with_db_pool(
        config_path,
        migrations_path_override,
        |pool, mpath, _| async move {
            crate::database::run_migrations(&pool, &mpath).await?;
            println!("{}", adapters::database::MIGRATIONS_APPLIED_MESSAGE);
            Ok(())
        },
    )
    .await
}

#[cfg(has_database)]
#[expect(clippy::print_stdout, reason = "CLI command output")]
pub async fn run_db_status(
    config_path: &str,
    migrations_path_override: Option<&str>,
) -> anyhow::Result<()> {
    with_db_pool(
        config_path,
        migrations_path_override,
        |pool, mpath, health_check_timeout| async move {
            let (health, counts_result) =
                crate::database::db_status_snapshot(&pool, &mpath, health_check_timeout).await;
            match counts_result {
                Ok(counts) => {
                    println!(
                        "{}",
                        adapters::database::format_db_status_report(&health, Ok(counts))
                    );
                    Ok(())
                }
                Err(e) => {
                    let msg = e.to_string();
                    println!(
                        "{}",
                        adapters::database::format_db_status_report(&health, Err(msg.as_str()))
                    );
                    Err(anyhow::Error::from(e))
                }
            }
        },
    )
    .await
}

#[cfg(test)]
#[path = "tests/cli_db_tests.rs"]
mod cli_db_tests;
#[cfg(test)]
#[path = "tests/cli_tests.rs"]
mod cli_tests;
