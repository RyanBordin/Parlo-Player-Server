//! Proxy local do lado do app.
//!
//! Escuta em `127.0.0.1` e leva cada conexão TCP até o PPS pelo túnel iroh.
//! O resto do app (e o player) só enxerga uma URL HTTP comum, como se o
//! Navidrome estivesse rodando na própria máquina.
//!
//! O Parlo Player embute este módulo; o binário `pps-client` deste
//! repositório é só uma casca de linha de comando em volta dele.

use std::{sync::Arc, time::Duration};

use anyhow::Context;
use iroh::{
    Endpoint, EndpointAddr,
    endpoint::{Connection, RecvStream, SendStream},
};
use tokio::{
    io::{AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Mutex,
};
use tracing::{debug, info, warn};

use crate::{
    ALPN,
    stream::{self, StreamKind, StreamStatus},
    ticket::ServerTicket,
};

/// Quanto esperar para estabelecer a conexão iroh antes de desistir.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// Quanto esperar pela confirmação do PPS ao abrir um stream. Passou disso,
/// a conexão é considerada morta.
const STATUS_TIMEOUT: Duration = Duration::from_secs(8);

pub struct LocalProxy {
    endpoint: Endpoint,
    server: EndpointAddr,
    connection: Mutex<Option<Connection>>,
}

impl LocalProxy {
    pub fn new(endpoint: Endpoint, ticket: &ServerTicket) -> Arc<Self> {
        Arc::new(Self {
            endpoint,
            server: ticket.endpoint_addr().clone(),
            connection: Mutex::new(None),
        })
    }

    /// Conecta ao PPS agora, em vez de esperar a primeira requisição. Útil
    /// para mostrar "conectado" / "servidor offline" logo ao abrir o app.
    pub async fn warm_up(&self) -> anyhow::Result<()> {
        self.connection().await.map(|_| ())
    }

    /// Aceita conexões em `listener` para sempre, cada uma numa task própria.
    pub async fn serve(self: Arc<Self>, listener: TcpListener) -> anyhow::Result<()> {
        loop {
            let (tcp, peer) = listener.accept().await?;
            debug!(%peer, "nova conexão local");
            let proxy = self.clone();
            tokio::spawn(async move {
                if let Err(err) = proxy.handle(tcp).await {
                    debug!(%peer, "conexão local encerrada com erro: {err:#}");
                }
            });
        }
    }

    /// Devolve a conexão iroh atual, reconectando se ela tiver caído.
    async fn connection(&self) -> anyhow::Result<Connection> {
        let mut guard = self.connection.lock().await;
        if let Some(conn) = guard.as_ref() {
            if conn.close_reason().is_none() {
                return Ok(conn.clone());
            }
            info!("conexão com o servidor caiu; reconectando");
        }
        let conn = tokio::time::timeout(
            CONNECT_TIMEOUT,
            self.endpoint.connect(self.server.clone(), ALPN),
        )
        .await
        .context("tempo esgotado conectando ao servidor")?
        .context("não foi possível conectar ao servidor")?;
        info!(server = %conn.remote_id().fmt_short(), "conectado ao servidor");
        *guard = Some(conn.clone());
        Ok(conn)
    }

    /// Abre um stream de música e espera o PPS confirmar.
    ///
    /// Se a conexão atual estiver morta sem que a gente saiba (o servidor
    /// caiu, a rede trocou), o status não chega: aí fecha a conexão e tenta
    /// uma vez com uma nova. Como nenhum byte do app foi enviado ainda, a
    /// segunda tentativa é transparente para o player.
    async fn open_music_stream(&self) -> anyhow::Result<OpenStream> {
        let conn = self.connection().await?;
        match open_on(&conn).await {
            Ok(opened) => Ok(opened),
            Err(err) => {
                debug!("stream falhou na conexão atual ({err:#}); tentando conexão nova");
                conn.close(0u32.into(), b"reconnect");
                let conn = self.connection().await?;
                open_on(&conn).await
            }
        }
    }

    async fn handle(&self, mut tcp: TcpStream) -> anyhow::Result<()> {
        let (mut send, mut recv, status) = match self.open_music_stream().await {
            Ok(opened) => opened,
            Err(err) => {
                warn!("servidor inalcançável: {err:#}");
                write_http_error(
                    &mut tcp,
                    "Servidor Parlo fora do ar ou inalcançável pelo túnel.",
                )
                .await?;
                return Ok(());
            }
        };
        match status {
            Ok(StreamStatus::Ok) => {}
            Ok(StreamStatus::BackendUnavailable) => {
                write_http_error(
                    &mut tcp,
                    "Servidor Parlo no ar, mas o servidor de música (Navidrome) não respondeu.",
                )
                .await?;
                return Ok(());
            }
            other => {
                write_http_error(
                    &mut tcp,
                    "Versão do servidor Parlo incompatível com este app.",
                )
                .await?;
                anyhow::bail!("status inesperado do servidor: {other:?}");
            }
        }

        let (mut tcp_read, mut tcp_write) = tcp.into_split();
        let upstream = async {
            tokio::io::copy(&mut tcp_read, &mut send).await?;
            send.finish()?;
            anyhow::Ok(())
        };
        let downstream = async {
            tokio::io::copy(&mut recv, &mut tcp_write).await?;
            tcp_write.shutdown().await?;
            anyhow::Ok(())
        };
        tokio::try_join!(upstream, downstream)?;
        Ok(())
    }
}

type OpenStream = (SendStream, RecvStream, Result<StreamStatus, u8>);

async fn open_on(conn: &Connection) -> anyhow::Result<OpenStream> {
    let (mut send, mut recv) = conn.open_bi().await?;
    stream::write_kind(&mut send, StreamKind::MusicProxy).await?;
    let status = tokio::time::timeout(STATUS_TIMEOUT, stream::read_status(&mut recv))
        .await
        .context("o servidor não respondeu a tempo")??;
    Ok((send, recv, status))
}

/// Responde um 502 em HTTP puro, para o player mostrar algo legível em vez
/// de só "conexão recusada".
async fn write_http_error<W: AsyncWrite + Unpin>(
    writer: &mut W,
    message: &str,
) -> std::io::Result<()> {
    let response = format!(
        "HTTP/1.1 502 Bad Gateway\r\n\
         Content-Type: text/plain; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {message}",
        message.len()
    );
    writer.write_all(response.as_bytes()).await?;
    writer.shutdown().await
}
