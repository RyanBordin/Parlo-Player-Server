# Docker para quem nunca usou

Este guia explica o Docker do zero, usando o próprio PPS como exemplo. No fim
tem o passo a passo para subir o Navidrome e o PPS juntos.

## A ideia em uma frase

O Docker deixa você rodar um programa **junto com tudo de que ele precisa**
(bibliotecas, versão certa do sistema, configurações) num pacote isolado, sem
instalar nada disso na sua máquina e sem um programa bagunçar o outro.

## Os conceitos, um de cada vez

### Imagem

Uma **imagem** é o pacote pronto: um "sistema de arquivos congelado" com o
programa e suas dependências. Ela não roda sozinha; é só o molde.

- `deluan/navidrome:latest` é a imagem oficial do Navidrome, baixada do
  [Docker Hub](https://hub.docker.com/r/deluan/navidrome) (uma espécie de "loja"
  de imagens).
- `parlo-player-server:dev` é a imagem do PPS, que **você mesmo constrói** a
  partir do código deste repositório.

Analogia: a imagem é como o instalador de um programa, só que já instalado e
configurado dentro de uma caixa.

### Container

Um **container** é uma imagem **rodando**. Dá para ter vários containers da
mesma imagem, cada um isolado: tem seus próprios arquivos, sua própria rede e
seus próprios processos. Se você apaga o container, apaga também tudo que ele
escreveu dentro de si (por isso existem os volumes, logo abaixo).

Analogia: se a imagem é a receita do bolo, o container é o bolo assado.

### Dockerfile

O **Dockerfile** é a receita para construir uma imagem. O do PPS
([`Dockerfile`](../Dockerfile)) faz duas etapas:

1. Parte de uma imagem que já tem o Rust instalado (`rust:1-slim-bookworm`),
   copia o código e compila (`cargo build --release`).
2. Parte de um Debian mínimo (`debian:bookworm-slim`) e copia **só o binário
   compilado** da etapa 1.

Isso se chama *multi-stage build*. A vantagem: a imagem final tem uns 150 MB
em vez de mais de 1 GB, porque o compilador fica para trás.

Cada linha do Dockerfile vira uma "camada". O Docker guarda as camadas em
cache: se você mudar só o código, ele não baixa o Debian de novo.

### Volume

Como o container perde o que escreveu quando é apagado, dados que precisam
durar ficam num **volume**: uma pasta que vive fora do container e é
"montada" dentro dele. Há dois tipos, e o nosso compose usa os dois:

- **Volume nomeado** (`pps-data`, `navidrome-data`): o Docker escolhe onde
  guardar e cuida dele. Bom para dados internos do programa. O PPS guarda ali a
  **chave secreta** do servidor; se ela se perder, o ticket muda e os apps
  precisam do ticket novo.
- **Bind mount** (`${MUSIC_DIR}:/music:ro`): uma pasta **sua**, que você
  escolhe, aparece dentro do container. É assim que o Navidrome enxerga suas
  músicas. O `:ro` significa *read only*: o Navidrome lê, mas não consegue
  alterar nada nas suas músicas.

### Portas

Cada container tem a própria rede, fechada. Para algo de fora alcançar um
container, você **publica** uma porta (`ports:` no compose), no formato
`porta-na-sua-máquina:porta-no-container`.

- `"127.0.0.1:4533:4533"` (Navidrome): publica a interface web **só para a
  própria máquina**. Do resto da rede e da internet, ninguém chega no
  Navidrome. É exatamente o que queremos: quem vem de fora passa pelo PPS.
- `"4535:4535/udp"` (PPS): a porta UDP do túnel iroh. Ela ajuda a conexão
  direta quando é possível. Atrás de CGNAT, o relay do iroh cobre o caminho e
  você **não precisa abrir nada no roteador**.

### Rede do compose

Todos os serviços de um mesmo `docker-compose.yml` ficam numa rede privada em
que **um acha o outro pelo nome**. Por isso o PPS fala com o Navidrome em
`navidrome:4533`: `navidrome` é o nome do serviço no compose, e não precisa de
IP nenhum.

### Docker Compose

O **Compose** é a ferramenta que sobe vários containers de uma vez a partir de
um arquivo ([`docker-compose.yml`](../docker-compose.yml)). Em vez de digitar
um comando `docker run` gigante para cada programa, você descreve tudo no
arquivo e roda `docker compose up`.

O nosso arquivo tem dois serviços (`navidrome` e `pps`), dois volumes
nomeados e as portas acima. O `restart: unless-stopped` faz os containers
voltarem sozinhos se o servidor reiniciar.

### O arquivo `.env`

O compose lê variáveis de um arquivo `.env` na mesma pasta. Usamos isso para a
pasta de músicas não ficar escrita no repositório: cada um coloca a sua.

## Passo a passo

### 1. Instalar o Docker (no servidor)

No Linux, o jeito mais simples é o script oficial:

```sh
curl -fsSL https://get.docker.com | sh
sudo usermod -aG docker $USER   # para usar o docker sem sudo
```

Saia e entre de novo na sessão (para o grupo valer) e teste:

```sh
docker run --rm hello-world
```

Se aparecer "Hello from Docker!", está funcionando. (Esse comando baixou uma
imagem minúscula, criou um container, rodou e apagou o container por causa do
`--rm`.)

### 2. Configurar a pasta de músicas

Na pasta do repositório:

```sh
cp .env.example .env
nano .env        # ajuste MUSIC_DIR para a pasta das suas músicas
```

### 3. Subir tudo

```sh
docker compose up -d --build
```

- `up`: cria e liga os containers.
- `-d` (*detached*): roda em segundo plano e devolve o terminal.
- `--build`: constrói a imagem do PPS a partir do código. Na primeira vez
  demora alguns minutos (compila o Rust); depois é rápido.

### 4. Criar o admin do Navidrome

Abra <http://localhost:4533> **no próprio servidor** (ou por um túnel SSH) e
crie o usuário administrador. Esse passo só acontece na primeira vez.

### 5. Pegar o ticket do servidor

```sh
docker compose logs pps
```

Procure a linha que começa com `endpoint...`. Esse é o **ticket**: o endereço
do seu servidor no iroh. Ele também fica salvo dentro do container:

```sh
docker compose exec pps cat /data/ticket.txt
```

(`exec` roda um comando dentro de um container que já está rodando.)

O ticket não muda entre reinícios, porque a chave fica no volume `pps-data`.

### 6. Comandos do dia a dia

| Comando | O que faz |
| --- | --- |
| `docker compose ps` | Lista os containers e se estão de pé |
| `docker compose logs -f pps` | Acompanha os logs do PPS ao vivo (Ctrl+C sai) |
| `docker compose restart pps` | Reinicia só o PPS |
| `docker compose down` | Para e remove os containers (**os volumes ficam**) |
| `docker compose up -d --build` | Recompila o PPS depois de um `git pull` e sobe de novo |
| `docker compose down -v` | Remove containers **e volumes**: apaga a chave do PPS e o banco do Navidrome. Cuidado! |

## Já tenho um Navidrome rodando fora do Docker

Dá para rodar só o PPS. Duas opções:

- **Sem Docker:** com o Rust instalado,
  `PPS_BACKEND=127.0.0.1:4533 cargo run --release -p pps-server`.
- **Com Docker, usando a rede da máquina:**

  ```sh
  docker build -t parlo-player-server:dev .
  docker run -d --name pps --network host \
    -e PPS_BACKEND=127.0.0.1:4533 \
    -v pps-data:/data \
    parlo-player-server:dev
  ```

  `--network host` faz o container usar a rede do servidor diretamente, então
  `127.0.0.1:4533` é o Navidrome que já está lá.
