//! Cliente de teste do PPS: abre um proxy em `127.0.0.1` que leva qualquer
//! app Subsonic até o servidor pelo túnel iroh.
//!
//! Uso: `pps-client <TICKET>` e depois aponte o app para
//! `http://127.0.0.1:4534`.

use std::{net::SocketAddr, path::PathBuf};

use clap::Parser;
use pps_protocol::{client::LocalProxy, ticket::ServerTicket};
use tokio::net::TcpListener;
use tracing::{info, warn};

#[derive(Debug, Parser)]
#[command(version, about)]
struct Args {
    /// Ticket impresso pelo servidor (também fica em data/ticket.txt).
    #[arg(env = "PPS_TICKET")]
    ticket: ServerTicket,

    /// Endereço local onde o app Subsonic vai se conectar.
    #[arg(long, env = "PPS_LISTEN", default_value = "127.0.0.1:4534")]
    listen: SocketAddr,

    /// Arquivo com a chave deste aparelho. Mantê-la fixa faz o servidor
    /// reconhecer o aparelho entre execuções (importa a partir da fase 2).
    #[arg(long, env = "PPS_CLIENT_KEY", default_value = "pps-client.key")]
    key: PathBuf,
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

    let secret_key = pps_protocol::identity::load_or_create(&args.key)?;
    info!("id deste aparelho: {}", secret_key.public());
    let endpoint = pps_protocol::bind_endpoint(secret_key, vec![], None).await?;

    let listener = TcpListener::bind(args.listen).await?;
    let proxy = LocalProxy::new(endpoint, &args.ticket);

    info!("conectando ao servidor...");
    if let Err(err) = proxy.warm_up().await {
        warn!("{err:#}; vou tentar de novo na primeira requisição");
    }

    println!(
        "\nPronto! No seu app Subsonic, use o servidor:\n\n    http://{}\n\n\
         com o seu usuário e senha do Navidrome. Ctrl+C para sair.\n",
        args.listen
    );

    tokio::select! {
        res = proxy.serve(listener) => res,
        _ = tokio::signal::ctrl_c() => Ok(()),
    }
}
