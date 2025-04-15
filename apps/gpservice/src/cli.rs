use std::io::Write;
use std::sync::Arc;

use anyhow::bail;
use clap::Parser;
use gpapi::clap::InfoLevelVerbosity;
use gpapi::logger;
use gpapi::{
    service::{request::WsRequest, vpn_state::VpnState},
    utils::{crypto::generate_key, lock_file::LockFile, redact::Redaction, shutdown_signal},
    GP_SERVICE_LOCK_FILE,
};
use log::info;
use tokio::sync::{mpsc, watch};

use crate::{vpn_task::VpnTask, ws_server::WsServer};

const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (",
    compile_time::date_str!(),
    ")"
);

#[derive(Parser)]
#[command(version = VERSION)]
struct Cli {
    #[clap(long)]
    minimized: bool,
    #[clap(long)]
    env_file: Option<String>,

    #[command(flatten)]
    verbose: InfoLevelVerbosity,
}

impl Cli {
    async fn run(&mut self) -> anyhow::Result<()> {
        let redaction = self.init_logger();
        info!("gpservice started: {}", VERSION);

        let pid = std::process::id();
        let lock_file = Arc::new(LockFile::new(GP_SERVICE_LOCK_FILE, pid));

        if lock_file.check_health().await {
            bail!("Another instance of the service is already running");
        }

        let api_key = self.prepare_api_key();

        // Channel for sending requests to the VPN task
        let (ws_req_tx, ws_req_rx) = mpsc::channel::<WsRequest>(32);
        // Channel for receiving the VPN state from the VPN task
        let (vpn_state_tx, vpn_state_rx) = watch::channel(VpnState::Disconnected);

        let mut vpn_task = VpnTask::new(ws_req_rx, vpn_state_tx);
        let ws_server = WsServer::new(
            api_key.clone(),
            ws_req_tx,
            vpn_state_rx,
            lock_file.clone(),
            redaction,
        );

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(4);
        let shutdown_tx_clone = shutdown_tx.clone();
        let vpn_task_cancel_token = vpn_task.cancel_token();
        let server_token = ws_server.cancel_token();

        #[cfg(unix)]
        {
            let vpn_ctx = vpn_task.context();
            let ws_ctx = ws_server.context();

            tokio::spawn(async move { signals::handle_signals(vpn_ctx, ws_ctx).await });
        }

        let vpn_task_handle = tokio::spawn(async move { vpn_task.start(server_token).await });
        let ws_server_handle =
            tokio::spawn(async move { ws_server.start(shutdown_tx_clone).await });

        tokio::select! {
          _ = shutdown_signal() => {
            info!("Shutdown signal received");
          }
          _ = shutdown_rx.recv() => {
            info!("Shutdown request received, shutting down");
          }
        }

        vpn_task_cancel_token.cancel();
        let _ = tokio::join!(vpn_task_handle, ws_server_handle);

        lock_file.unlock()?;

        info!("gpservice stopped");

        Ok(())
    }

    fn init_logger(&self) -> Arc<Redaction> {
        let redaction = Arc::new(Redaction::new());
        let redaction_clone = Arc::clone(&redaction);

        let inner_logger = env_logger::builder()
            // Set the log level to the Trace level, the logs will be filtered
            .filter_level(log::LevelFilter::Trace)
            .format(move |buf, record| {
                let timestamp = buf.timestamp();
                writeln!(
                    buf,
                    "[{} {}  {}] {}",
                    timestamp,
                    record.level(),
                    record.module_path().unwrap_or_default(),
                    redaction_clone.redact_str(&record.args().to_string())
                )
            })
            .build();

        let level = self
            .verbose
            .log_level_filter()
            .to_level()
            .unwrap_or(log::Level::Info);

        logger::init_with_logger(level, inner_logger);

        redaction
    }

    fn prepare_api_key(&self) -> Vec<u8> {
        generate_key().to_vec()
    }
}

#[cfg(unix)]
mod signals {
    use std::sync::Arc;

    use log::{info, warn};

    use crate::vpn_task::VpnTaskContext;
    use crate::ws_server::WsServerContext;

    const DISCONNECTED_PID_FILE: &str = "/tmp/gpservice_disconnected.pid";

    pub async fn handle_signals(vpn_ctx: Arc<VpnTaskContext>, ws_ctx: Arc<WsServerContext>) {
        use gpapi::service::event::WsEvent;
        use tokio::signal::unix::{signal, Signal, SignalKind};

        let (mut user_sig1, mut user_sig2) = match || -> anyhow::Result<(Signal, Signal)> {
            let user_sig1 = signal(SignalKind::user_defined1())?;
            let user_sig2 = signal(SignalKind::user_defined2())?;
            Ok((user_sig1, user_sig2))
        }() {
            Ok(signals) => signals,
            Err(err) => {
                warn!("Failed to create signal: {}", err);
                return;
            }
        };

        loop {
            tokio::select! {
              _ = user_sig1.recv() => {
                info!("Received SIGUSR1 signal");
                if vpn_ctx.disconnect().await {
                  // Write the PID to a dedicated file to indicate that the VPN task is disconnected via SIGUSR1
                  let pid = std::process::id();
                  if let Err(err) = tokio::fs::write(DISCONNECTED_PID_FILE, pid.to_string()).await {
                    warn!("Failed to write PID to file: {}", err);
                  }
                }
              }
              _ = user_sig2.recv() => {
                info!("Received SIGUSR2 signal");
                ws_ctx.send_event(WsEvent::ResumeConnection).await;
              }
            }
        }
    }
}

pub async fn run() {
    let mut cli = Cli::parse();

    if let Err(e) = cli.run().await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
