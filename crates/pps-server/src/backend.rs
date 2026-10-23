//! Backend de música: para onde o PPS manda os bytes que chegam pelo túnel.
//!
//! Hoje só existe o [`Navidrome`], mas o túnel não sabe disso: ele só pede
//! "me dá uma conexão com o backend". Trocar o Navidrome por outro servidor
//! (outro compatível com OpenSubsonic, ou uma biblioteca própria em Rust
//! rodando no mesmo processo) é implementar esta trait.

use std::{fmt, future::Future, io};

use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};

pub trait MusicBackend: fmt::Debug + Send + Sync + 'static {
    /// O canal de bytes até o backend. Para servidores externos é um
    /// `TcpStream`; um backend embutido poderia usar `tokio::io::duplex`.
    type Stream: AsyncRead + AsyncWrite + Unpin + Send + 'static;

    /// Abre um canal novo até o backend (um por stream do túnel).
    fn connect(&self) -> impl Future<Output = io::Result<Self::Stream>> + Send;
}

/// Navidrome (ou qualquer servidor HTTP) ouvindo num endereço TCP.
#[derive(Debug, Clone)]
pub struct Navidrome {
    addr: String,
}

impl Navidrome {
    /// `addr` no formato `host:porta`, por exemplo `127.0.0.1:4533` ou, dentro
    /// do docker compose, `navidrome:4533`.
    pub fn new(addr: impl Into<String>) -> Self {
        Self { addr: addr.into() }
    }
}

impl fmt::Display for Navidrome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Navidrome em {}", self.addr)
    }
}

impl MusicBackend for Navidrome {
    type Stream = TcpStream;

    async fn connect(&self) -> io::Result<TcpStream> {
        let stream = TcpStream::connect(&self.addr).await?;
        stream.set_nodelay(true)?;
        Ok(stream)
    }
}
