//! Lado do servidor do túnel: aceita conexões iroh e liga cada stream ao
//! backend de música.

use std::sync::Arc;

use iroh::{
    endpoint::{Connection, RecvStream, SendStream},
    protocol::{AcceptError, ProtocolHandler},
};
use pps_protocol::stream::{self, StreamKind, StreamStatus};
use tokio::io::AsyncWriteExt;
use tracing::{debug, info, warn};

use crate::backend::MusicBackend;

#[derive(Debug)]
pub struct Tunnel<B> {
    backend: Arc<B>,
}

impl<B: MusicBackend> Tunnel<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend: Arc::new(backend),
        }
    }
}

impl<B: MusicBackend> ProtocolHandler for Tunnel<B> {
    // Fase 1: qualquer aparelho com o ticket entra. Na fase 2, a checagem da
    // lista de aparelhos autorizados entra em `on_accepting`, recusando com
    // `AcceptError::NotAllowed` antes de qualquer stream ser aberto.

    async fn accept(&self, connection: Connection) -> Result<(), AcceptError> {
        let device = connection.remote_id();
        info!(device = %device.fmt_short(), "aparelho conectado");

        loop {
            let (send, recv) = match connection.accept_bi().await {
                Ok(streams) => streams,
                Err(err) => {
                    info!(device = %device.fmt_short(), "aparelho desconectado: {err}");
                    return Ok(());
                }
            };
            let backend = self.backend.clone();
            tokio::spawn(async move {
                if let Err(err) = handle_stream(backend.as_ref(), send, recv).await {
                    debug!(device = %device.fmt_short(), "stream encerrado com erro: {err:#}");
                }
            });
        }
    }
}

async fn handle_stream<B: MusicBackend>(
    backend: &B,
    mut send: SendStream,
    mut recv: RecvStream,
) -> anyhow::Result<()> {
    match stream::read_kind(&mut recv).await? {
        Ok(StreamKind::MusicProxy) => {}
        Err(unknown) => {
            warn!("aparelho pediu tipo de stream desconhecido: {unknown}");
            stream::write_status(&mut send, StreamStatus::UnsupportedKind).await?;
            send.finish()?;
            return Ok(());
        }
    }

    let backend_stream = match backend.connect().await {
        Ok(stream) => stream,
        Err(err) => {
            warn!("backend de música inalcançável: {err}");
            stream::write_status(&mut send, StreamStatus::BackendUnavailable).await?;
            send.finish()?;
            return Ok(());
        }
    };
    stream::write_status(&mut send, StreamStatus::Ok).await?;

    let (mut backend_read, mut backend_write) = tokio::io::split(backend_stream);

    let upstream = async {
        tokio::io::copy(&mut recv, &mut backend_write).await?;
        backend_write.shutdown().await?;
        anyhow::Ok(())
    };
    let downstream = async {
        tokio::io::copy(&mut backend_read, &mut send).await?;
        send.finish()?;
        anyhow::Ok(())
    };

    tokio::try_join!(upstream, downstream)?;
    Ok(())
}
