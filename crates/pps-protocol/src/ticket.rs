//! Ticket do servidor: o texto que o app precisa para achar o PPS.
//!
//! Hoje é só o endereço iroh (chave pública + dicas de como chegar: relay e
//! IPs). Na fase 2 ele vira a base do link `parlo://convite/...`, ganhando o
//! token de convite de uso único.

use std::{fmt, str::FromStr};

use iroh::EndpointAddr;
use iroh_tickets::{Ticket, endpoint::EndpointTicket};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerTicket(EndpointTicket);

impl ServerTicket {
    pub fn new(addr: EndpointAddr) -> Self {
        Self(EndpointTicket::new(addr))
    }

    pub fn endpoint_addr(&self) -> &EndpointAddr {
        self.0.endpoint_addr()
    }
}

impl fmt::Display for ServerTicket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.encode_string())
    }
}

impl FromStr for ServerTicket {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ticket = EndpointTicket::decode_string(s.trim())
            .map_err(|err| anyhow::anyhow!("ticket inválido: {err}"))?;
        Ok(Self(ticket))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let key = iroh::SecretKey::generate();
        let ticket = ServerTicket::new(EndpointAddr::new(key.public()));
        let parsed: ServerTicket = ticket.to_string().parse().unwrap();
        assert_eq!(parsed, ticket);
    }

    #[test]
    fn rejects_garbage() {
        assert!("isso não é um ticket".parse::<ServerTicket>().is_err());
    }
}
