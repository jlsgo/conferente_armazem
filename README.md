# Controle de Armazem

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

#### Para usar o aplicativo pronto

O usuario final **nao precisa instalar Rust, Node.js ou Build Tools**. Baixe o
instalador Windows (`.msi` ou `.exe`) na area de releases do projeto, execute-o e
siga o assistente. O WebView2 ja vem no Windows 10/11 atualizado; se estiver
ausente, instale-o pelo [site oficial da Microsoft](https://developer.microsoft.com/microsoft-edge/webview2/).

#### Para desenvolver ou compilar o aplicativo

Abra o **PowerShell como Administrador** e instale as dependencias com `winget`:

```powershell
winget install --id OpenJS.NodeJS.LTS --exact
winget install --id Rustlang.Rustup --exact
winget install --id Microsoft.VisualStudio.2022.BuildTools --exact `
  --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools;includeRecommended"
```

Feche e abra o PowerShell novamente. Instale as dependencias do projeto:

```powershell
git clone <url-do-repositorio>
Set-Location controle_de_saidas_e_entradas_dos_armazens_ecoviva
npm install
```

#### Executar uma copia trazida por pendrive

Copie a pasta do projeto para o computador Windows. Nao e necessario copiar as
pastas `node_modules` e `src-tauri/target`; elas sao especificas do computador.
Depois abra o PowerShell como Administrador e cole o bloco abaixo:

```powershell
# Instala as ferramentas necessarias, se ainda nao estiverem instaladas.
winget install --id OpenJS.NodeJS.LTS --exact
winget install --id Rustlang.Rustup --exact
winget install --id Microsoft.VisualStudio.2022.BuildTools --exact `
  --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools;includeRecommended"

Write-Host "Instalacao concluida. Feche este PowerShell, abra outro e cole o proximo bloco." -ForegroundColor Green
```

No novo PowerShell, cole:

```powershell
$pastaProjeto = Read-Host "Informe o caminho da pasta do projeto no pendrive ou no PC"

if (-not (Test-Path (Join-Path $pastaProjeto "package.json"))) {
  throw "package.json nao encontrado. Informe a pasta raiz do projeto."
}

Set-Location $pastaProjeto
npm install
npx tauri info
npm run dev
```

Exemplo de caminho valido:

```text
E:\controle_de_saidas_e_entradas_dos_armazens_ecoviva
```

O comando `npm install` deve ser executado no Windows mesmo que a pasta tenha
vindo de outro sistema operacional. O aplicativo sera aberto com `npm run dev`.

Teste o ambiente e execute o aplicativo:

```powershell
npx tauri info
npm run dev
```

Se o `winget` nao estiver disponivel, instale o **App Installer** pela Microsoft
Store ou use os instaladores oficiais de [Node.js](https://nodejs.org/),
[Rust](https://www.rust-lang.org/tools/install) e [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/).

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
