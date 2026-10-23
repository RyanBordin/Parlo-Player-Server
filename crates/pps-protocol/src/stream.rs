//! Cabeçalho de cada stream do túnel.
//!
//! Formato (fase 1):
//!
//! ```text
//! app -> PPS : [kind: u8]   e depois os bytes da conexão
//! PPS -> app : [status: u8] e depois os bytes da resposta
//! ```
//!
//! É propositalmente mínimo. Novos tipos de stream (por exemplo, o canal de
//! controle de convites da fase 2) entram como novos valores de
//! [`StreamKind`] sem quebrar os antigos.

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// O que o app quer fazer com este stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum StreamKind {
    /// Encaminhar bytes crus para o backend de música (HTTP/Subsonic).
    MusicProxy = 1,
}

/// Resposta do PPS ao abrir um stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum StreamStatus {
    /// Tudo certo: daqui para frente os bytes vêm do backend.
    Ok = 0,
    /// O PPS está no ar, mas não conseguiu falar com o backend (Navidrome).
    BackendUnavailable = 1,
    /// O PPS não conhece o [`StreamKind`] pedido (versões diferentes).
    UnsupportedKind = 2,
}

impl TryFrom<u8> for StreamKind {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::MusicProxy),
            other => Err(other),
        }
    }
}

impl TryFrom<u8> for StreamStatus {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Ok),
            1 => Ok(Self::BackendUnavailable),
            2 => Ok(Self::UnsupportedKind),
            other => Err(other),
        }
    }
}

/// Lê o tipo de stream pedido. Devolve `Err(byte)` dentro do `Ok` externo
/// quando o valor é desconhecido, para o PPS poder responder
/// [`StreamStatus::UnsupportedKind`].
pub async fn read_kind<R: AsyncRead + Unpin>(
    reader: &mut R,
) -> std::io::Result<Result<StreamKind, u8>> {
    Ok(StreamKind::try_from(reader.read_u8().await?))
}

pub async fn write_kind<W: AsyncWrite + Unpin>(
    writer: &mut W,
    kind: StreamKind,
) -> std::io::Result<()> {
    writer.write_u8(kind as u8).await
}

pub async fn read_status<R: AsyncRead + Unpin>(
    reader: &mut R,
) -> std::io::Result<Result<StreamStatus, u8>> {
    Ok(StreamStatus::try_from(reader.read_u8().await?))
}

pub async fn write_status<W: AsyncWrite + Unpin>(
    writer: &mut W,
    status: StreamStatus,
) -> std::io::Result<()> {
    writer.write_u8(status as u8).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn kind_round_trip() {
        let mut buf = Vec::new();
        write_kind(&mut buf, StreamKind::MusicProxy).await.unwrap();
        let kind = read_kind(&mut buf.as_slice()).await.unwrap();
        assert_eq!(kind, Ok(StreamKind::MusicProxy));
    }

    #[tokio::test]
    async fn status_round_trip() {
        for status in [
            StreamStatus::Ok,
            StreamStatus::BackendUnavailable,
            StreamStatus::UnsupportedKind,
        ] {
            let mut buf = Vec::new();
            write_status(&mut buf, status).await.unwrap();
            assert_eq!(read_status(&mut buf.as_slice()).await.unwrap(), Ok(status));
        }
    }

    #[tokio::test]
    async fn unknown_kind_is_reported() {
        let kind = read_kind(&mut [42u8].as_slice()).await.unwrap();
        assert_eq!(kind, Err(42));
    }
}
