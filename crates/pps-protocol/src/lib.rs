//! Protocolo do túnel do Parlo Player Server (PPS).
//!
//! Este crate é o único ponto de contato entre o PPS e os apps Parlo: os dois
//! lados dependem dele para falar o mesmo "idioma" por cima do iroh.
//!
//! - [`ALPN`]: identifica o protocolo na conexão iroh (QUIC).
//! - [`stream`]: o mini-cabeçalho trocado no começo de cada stream.
//! - [`ticket`]: o "endereço" do servidor que o app precisa para conectar.
//! - [`identity`]: guarda a chave secreta de cada lado em disco.
//! - [`client`]: o proxy local (`127.0.0.1`) que os apps embutem.
//!
//! ## Como o túnel funciona (fase 1)
//!
//! O app abre **uma** conexão iroh com o PPS. Cada conexão TCP que o player
//! faz no proxy local vira **um** stream bidirecional dentro dessa conexão.
//! O primeiro byte do stream diz o que o app quer ([`stream::StreamKind`]);
//! o PPS responde com um byte de status ([`stream::StreamStatus`]). Só depois
//! do status o app começa a mandar dados; a partir daí, os bytes passam crus
//! nos dois sentidos até o backend de música.

pub mod client;
pub mod identity;
pub mod stream;
pub mod ticket;

pub use iroh;

/// ALPN do protocolo. Ambos os lados precisam anunciar exatamente o mesmo
/// valor, senão o iroh recusa a conexão. Mudanças incompatíveis no protocolo
/// trocam o número no final.
pub const ALPN: &[u8] = b"parlo/pps/0";

/// Cria um endpoint iroh com as configurações padrão do Parlo.
///
/// Usa os relays públicos do n0 (preset `N0`), que bastam para a prova de
/// conceito. Na fase 4 entra a opção de relay próprio.
///
/// `udp_port` fixa a porta UDP (IPv4) em vez de deixar o sistema escolher
/// uma aleatória. O servidor usa isso para o endereço não mudar entre
/// reinícios e para a porta poder ser publicada no Docker.
pub async fn bind_endpoint(
    secret_key: iroh::SecretKey,
    accept_alpns: Vec<Vec<u8>>,
    udp_port: Option<u16>,
) -> anyhow::Result<iroh::Endpoint> {
    let mut builder = iroh::Endpoint::builder(iroh::endpoint::presets::N0)
        .secret_key(secret_key)
        .alpns(accept_alpns);
    if let Some(port) = udp_port {
        builder = builder.bind_addr(std::net::SocketAddr::from(([0, 0, 0, 0], port)))?;
    }
    Ok(builder.bind().await?)
}
