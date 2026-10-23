//! Parlo Player Server (PPS), fase 1: prova de conceito do túnel.
//!
//! Abre um endpoint iroh, imprime o ticket e encaminha tudo que chega pelo
//! túnel para o Navidrome.

mod backend;
mod tunnel;

use std::{path::PathBuf, time::Duration};

use clap::Parser;
use iroh::{Endpoint, protocol::Router};
use pps_protocol::{ALPN, ticket::ServerTicket};
use tracing::{error, info, warn};

use crate::{
    backend::{MusicBackend, Navidrome},
    tunnel::Tunnel,
};

#[derive(Debug, Parser)]
#[command(version, about)]
struct Args {
    /// Endereço (host:porta) do Navidrome.
    #[arg(long, env = "PPS_BACKEND", default_value = "127.0.0.1:4533")]
    backend: String,

    /// Pasta onde o PPS guarda a chave secreta e o ticket.
    #[arg(long, env = "PPS_DATA_DIR", default_value = "data")]
    data_dir: PathBuf,

    /// Porta UDP fixa para o túnel. Sem ela, o sistema escolhe uma aleatória
    /// a cada início.
    #[arg(long, env = "PPS_UDP_PORT")]
    udp_port: Option<u16>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,iroh=warn,noq=warn".into()),
        )
        .init();
    let args = Args::parse();

    let secret_key = pps_protocol::identity::load_or_create(&args.data_dir.join("secret.key"))?;
    let endpoint =
        pps_protocol::bind_endpoint(secret_key, vec![ALPN.to_vec()], args.udp_port).await?;

    let backend = Navidrome::new(&args.backend);
    match backend.connect().await {
        Ok(_) => info!("{backend} respondeu"),
        Err(err) => warn!("{backend} não respondeu ainda ({err}); seguindo mesmo assim"),
    }

    // Começa a aceitar conexões já; o ticket sai depois, quando o endpoint
    // souber os próprios endereços.
    let router = Router::builder(endpoint)
        .accept(ALPN, Tunnel::new(backend))
        .spawn();

    // Anuncia o ticket em segundo plano, para um `docker stop` durante a
    // espera pelo relay não ficar sem resposta.
    let announce = tokio::spawn(announce_ticket(
        router.endpoint().clone(),
        args.data_dir.join("ticket.txt"),
    ));

    shutdown_signal().await;
    info!("encerrando");
    announce.abort();
    router.shutdown().await?;
    Ok(())
}

/// Espera o endpoint descobrir seus endereços e então grava e imprime o ticket.
async fn announce_ticket(endpoint: Endpoint, ticket_path: PathBuf) {
    if tokio::time::timeout(Duration::from_secs(15), endpoint.online())
        .await
        .is_err()
    {
        warn!("ainda sem contato com um relay; o ticket pode não ter o endereço do relay");
    }

    let ticket = ServerTicket::new(endpoint.addr());
    if let Err(err) = std::fs::write(&ticket_path, format!("{ticket}\n")) {
        error!("não foi possível gravar {}: {err}", ticket_path.display());
    } else {
        info!("ticket salvo em {}", ticket_path.display());
    }
    info!("id do servidor: {}", endpoint.id());
    println!("\nTicket do servidor (cole no pps-client):\n\n{ticket}\n");
}

/// Espera Ctrl+C ou SIGTERM (que é o que o Docker manda no `docker stop`).
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut term = signal(SignalKind::terminate()).expect("registrando SIGTERM");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
