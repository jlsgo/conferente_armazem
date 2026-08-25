# Ecoviva - Controle de Armazem

App desktop offline-first (Tauri v2 + Rust + SQLite, frontend React/TypeScript) para
registrar as entradas e saidas dos armazens A4 e B2 da Ecoviva, substituindo as
planilhas de papel/Excel usadas pelas conferentes.

Veja `docs/ARQUITETURA.md` para a arquitetura completa e `docs/ROADMAP.md` para o que
ja esta pronto e o que vem a seguir.

## Pre-requisitos

- [Node.js](https://nodejs.org/) 20+
- [Rust](https://www.rust-lang.org/tools/install) estavel (>= 1.77.2)
- Dependencias de sistema do Tauri (variam por SO, ver abaixo)

### Linux (Ubuntu/Debian)

```bash
sudo apt-get update
sudo apt-get install -y build-essential curl wget file libssl-dev \
  libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

# Node.js 20 (via nvm, se ainda nao tiver Node instalado)
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.1/install.sh | bash
nvm install 20
```

### Windows

1. Instale o [Rust](https://www.rust-lang.org/tools/install) (`rustup-init.exe`,
   toolchain padrao MSVC).
2. Instale o **Microsoft C++ Build Tools**: baixe o
   [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
   e marque o pacote "Desktop development with C++".
3. Instale o [Node.js](https://nodejs.org/) 20+.
4. O **WebView2** ja vem instalado no Windows 10/11 atualizados; se faltar, o
   instalador pede para baixar automaticamente.

Depois disso os comandos abaixo (PowerShell/CMD) sao os mesmos do Linux.

## Instalar e rodar

```bash
git clone <url-do-repositorio>
cd controle_de_saidas_e_entradas_dos_armazens_ecoviva
npm install
npm run dev        # abre a janela do app com hot-reload do frontend
```

No primeiro uso, a tela de "Setup" pede para criar o primeiro usuario (vira gestor
automaticamente) — nao ha usuario pre-cadastrado.

## Outros comandos uteis

Frontend (raiz do repo):

```bash
npm run build:renderer   # build do frontend (vite build), so gera dist/
npx tsc --noEmit          # type-check do frontend
npm run dist               # gera o instalador da plataforma atual (tauri build)
```

Backend (`src-tauri/`):

```bash
cd src-tauri
cargo test                                          # testes de dominio + integracao
cargo test <trecho_do_nome>                          # roda so um teste
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt                                            # formata (--check so verifica)
cargo check                                          # type-check rapido, sem build completo
```

## CI

`.github/workflows/ci.yml` roda `fmt --check`, `clippy`, `cargo test`, `tsc --noEmit`
e o build do frontend em `ubuntu-latest` e `windows-latest` a cada push/PR na `main`
— Windows e o alvo real de distribuicao, Linux e o que este tipo de ambiente de
desenvolvimento consegue buildar e rodar localmente.
