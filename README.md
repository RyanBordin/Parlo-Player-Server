# Parlo Player Server (PPS)

Servidor que deixa amigos e família ouvirem o seu Navidrome de qualquer lugar,
**sem VPN, sem abrir porta no roteador e funcionando atrás de CGNAT**. O
Navidrome continua fechado para a internet; só quem passa pelo túnel do PPS
chega nele.

> **Fase atual: 1, prova de conceito do túnel.** O PPS só encaminha o tráfego.
> Ainda não há convites, contas nem lista de aparelhos: **qualquer pessoa com o
> ticket consegue conectar** (e ainda precisa de usuário e senha do Navidrome).
> Não divulgue o ticket por enquanto.

## Como funciona

```text
 Fora de casa                                       Em casa (servidor)
┌──────────────────────────────┐                  ┌──────────────────────────────┐
│ App Subsonic / Parlo Player  │                  │  Navidrome (localhost:4533)  │
│        │ HTTP comum          │                  │        ▲ HTTP comum          │
│        ▼                     │   túnel iroh     │        │                     │
│ proxy local 127.0.0.1:4534 ══╪══ QUIC cifrado ══╪══> PPS │                     │
│   (pps-client ou embutido)   │ direto ou relay  │                              │
└──────────────────────────────┘                  └──────────────────────────────┘
```

- O túnel usa o [iroh](https://iroh.computer): cada lado é identificado por
  uma chave pública. Ele tenta conexão direta (com *hole punching*) e, se os
  dois lados estiverem atrás de NAT restritivo, passa por um relay. Nesta fase
  usamos os relays públicos do n0.
- O **ticket** é o "endereço" do servidor: a chave pública dele mais dicas de
  como chegar. O app precisa dele para conectar.
- Cada conexão TCP que o player abre no proxy local vira um stream dentro da
  conexão iroh. O PPS liga esse stream no Navidrome. O túnel não interpreta o
  HTTP, então streaming, *range requests*, capas e a interface web funcionam
  sem nada especial.

## Organização do código

| Crate | O que é |
| --- | --- |
| [`crates/pps-protocol`](crates/pps-protocol) | O "idioma" do túnel, compartilhado com o Parlo Player: ALPN, cabeçalho de stream, ticket, chave do aparelho e o **proxy local** (`client::LocalProxy`) que o app embute. |
| [`crates/pps-server`](crates/pps-server) | O servidor (binário `pps`). O backend de música é uma trait (`MusicBackend`), hoje implementada pelo Navidrome, para poder ser trocado no futuro. |
| [`crates/pps-client`](crates/pps-client) | Casca de linha de comando em volta do `LocalProxy`, para testar com qualquer app Subsonic antes da integração no Parlo. |

O Parlo Player pode usar o protocolo como dependência git:

```toml
pps-protocol = { git = "https://github.com/RyanBordin/Parlo-Player-Server" }
```

## Testando a fase 1

### No servidor (em casa)

Siga o [guia de Docker](docs/docker.md). Em resumo:

```sh
cp .env.example .env              # e ajuste MUSIC_DIR
docker compose up -d --build
docker compose logs pps           # copie o ticket (linha "endpoint...")
```

Na primeira vez, crie o admin do Navidrome em <http://localhost:4533> no
próprio servidor.

### No notebook (fora de casa)

Para ter certeza de que está passando pelo túnel, use o 4G do celular como
hotspot. Com o Rust instalado:

```sh
cargo run --release -p pps-client -- <TICKET>
```

Ele fica escutando em `http://127.0.0.1:4534`. Aponte o app Subsonic (ou o
Parlo Player) para esse endereço, com seu usuário e senha do Navidrome. A
interface web do Navidrome também abre nesse endereço pelo navegador.

Se algo der errado, o proxy responde um erro HTTP 502 com uma mensagem legível:

- *"Servidor Parlo fora do ar ou inalcançável pelo túnel."*: o PPS não está
  rodando ou não foi possível chegar nele.
- *"Servidor Parlo no ar, mas o servidor de música (Navidrome) não
  respondeu."*: o túnel funcionou, o problema é no Navidrome.

### Configuração

| Variável | Padrão | Para quê |
| --- | --- | --- |
| `PPS_BACKEND` | `127.0.0.1:4533` (`navidrome:4533` no Docker) | Onde está o Navidrome |
| `PPS_DATA_DIR` | `data` (`/data` no Docker) | Onde ficam `secret.key` e `ticket.txt` |
| `PPS_UDP_PORT` | aleatória (`4535` no Docker) | Porta UDP fixa do túnel |
| `PPS_LISTEN` (cliente) | `127.0.0.1:4534` | Endereço local do proxy |
| `PPS_CLIENT_KEY` (cliente) | `pps-client.key` | Chave deste aparelho |
| `RUST_LOG` | `info,iroh=warn,noq=warn` | Nível de log (ex.: `debug`) |

## Próximas fases

1. ~~Prova de conceito do túnel~~ (esta)
2. Convites e contas: token de convite, link `parlo://convite/...`, criação de
   usuário no Navidrome, aparelhos autorizados e revogação.
3. App Android (no repositório do Parlo Player).
4. Relay próprio e acesso opcional por URL.
5. Funções de família.

## Desenvolvimento

```sh
cargo test
cargo clippy --all-targets
cargo fmt
```
