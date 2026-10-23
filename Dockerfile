# Imagem Docker do Parlo Player Server (PPS).
#
# Este arquivo é a "receita" da imagem. Ele tem dois estágios:
#   1. "build": uma imagem grande, com o Rust inteiro, só para compilar.
#   2. a imagem final: um Debian mínimo com apenas o binário pronto.
# Assim a imagem que você roda fica pequena e sem compilador dentro.
# Explicação completa em docs/docker.md.

# ---- Estágio 1: compilar ----
FROM rust:1-slim-bookworm AS build
WORKDIR /src

# Copia o código para dentro da imagem de build e compila em modo release.
COPY . .
RUN cargo build --release --locked -p pps-server

# ---- Estágio 2: imagem final ----
FROM debian:bookworm-slim

# Certificados raiz, para o PPS conseguir falar HTTPS com os serviços do
# iroh. A imagem do Rust já tem; copiamos de lá em vez de instalar.
COPY --from=build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt

# Roda como um usuário comum, não como root, por segurança.
RUN useradd --system --uid 10001 --home-dir /data pps \
    && mkdir /data && chown pps /data
USER pps

COPY --from=build /src/target/release/pps /usr/local/bin/pps

# Valores padrão; o docker-compose.yml pode sobrescrever.
ENV PPS_DATA_DIR=/data \
    PPS_BACKEND=navidrome:4533 \
    PPS_UDP_PORT=4535

# /data guarda a chave secreta do servidor: ela precisa sobreviver quando o
# container é recriado, senão o ticket muda.
VOLUME /data

# Porta UDP do túnel iroh (documentação; quem publica é o compose).
EXPOSE 4535/udp

ENTRYPOINT ["/usr/local/bin/pps"]
