# Arquitetura

Sistema desktop **offline-first** para as conferentes dos armazens Ecoviva registrarem
entradas e saidas (patinetes, scooters, triciclos e pecas), substituindo as planilhas em
papel/Excel. Roda inteiramente no computador de cada armazem, sem depender de internet.

## Stack

- **Backend**: Rust + [Tauri v2](https://tauri.app). Um unico binario nativo por
  plataforma (Windows e Linux hoje; macOS funciona pela mesma base, sem trabalho extra).
- **Banco de dados**: SQLite (via `rusqlite`, com o driver compilado junto ao app —
  `features = ["bundled"]"`, sem exigir SQLite instalado no PC). Um arquivo por
  computador, em `<diretorio de dados do usuario>/ecoviva-armazem.db`.
- **Frontend**: React + TypeScript, empacotado com Vite e servido pelo proprio Tauri
  (sem servidor HTTP externo).

## Por que Tauri (e nao Electron)

O protótipo inicial foi feito em Electron. Foi trocado por Tauri ainda no começo do
projeto porque: o instalador final fica muito menor, o backend em Rust é memory-safe, o
sistema de `capabilities` do Tauri aplica menor privilégio de forma declarativa
(`src-tauri/capabilities/default.json` — nenhum plugin de `fs`, `shell`, `http` ou
`dialog` habilitado, so os comandos custom da aplicacao), e a arvore de dependencias e
muito menor (o `npm audit` do projeto em Electron acusava 15 vulnerabilidades, 1 critica;
com Tauri, 0).

## Modulos (`src-tauri/src`)

```
domain/     Regras de negocio puras. Nao conhece Tauri nem SQL de UI - so recebe uma
            &Connection/&mut Connection e retorna Result<T, AppError>. E aqui que fica
            toda validacao. Testado com banco SQLite em memoria (rapido, sem mocks).
  auth.rs        hash/verify de senha (Argon2), criar usuario, login.
  movimentos.rs  criar pedido com N itens, listar o dia, sugestoes de descricao,
                 cadeia de hash de auditoria (SHA-256), autorizacao de leitura por
                 sessao/armazem (`autorizar_leitura`) alem da de escrita.
  errors.rs      AppError (thiserror) - unica fonte de mensagens de erro mostradas
                 na tela; nunca vaza detalhe de SQL para o frontend.

db/         Abertura da conexao SQLite, pragmas (WAL, foreign_keys), aplicacao das
            migrations e seed dos dois armazens (A4/B2).
  backup.rs      backup automatico diario (local e externo) com retencao, e checagem
                 de integridade de um arquivo de backup (ver secao propria abaixo).
  sync.rs        sincronizacao oportunista com o Turso (ver secao propria abaixo).

commands/   Wrappers finos com #[tauri::command]. So extraem o State, chamam
            domain::* e devolvem o Result. Nao tem logica de negocio aqui de proposito
            - é a camada que fala com o frontend via IPC.

state.rs    AppState: uma unica Mutex<Connection> (ver "Por que nao um pool" abaixo).
```

O frontend (`src/`) chama tudo atraves de `src/lib/api.ts`, que envolve
`@tauri-apps/api` `invoke()` com tipos TypeScript espelhando as structs Rust
(`src/types.ts`). As paginas (`src/pages/*.tsx`) nao conhecem `invoke` diretamente.

## Por que nao ha catalogo de produtos

O catalogo fixo (modelo+cor com chave estrangeira obrigatoria) foi tentado e descartado:
a saida real cobre scooters, triciclos, patinetes e pecas — variedade grande demais pra
manter uma lista fechada, e pedidos de veiculo ja tem o detalhe completo em outra
ferramenta da empresa (o numero do pedido). Por isso `movimento_itens.descricao` e texto
livre opcional, com sugestoes de autocompletar vindas dos proprios lancamentos anteriores
(`domain::movimentos::sugestoes_descricao`), nao de uma tabela mantida a parte.

## Por que uma unica conexao (Mutex) em vez de um pool

E uma app desktop de uso local, tipicamente 1-2 conferentes por armazem digitando por
vez — nao um servidor web com centenas de conexoes simultaneas. Um pool (`r2d2` ou
similar) so adicionaria uma dependencia e complexidade sem beneficio real nesse cenario.
Se um dia isso mudar (varios usuarios simultaneos no mesmo PC, por exemplo), trocar por
um pool e uma mudanca isolada em `state.rs`.

## Fechamento do dia: sem biblioteca de PDF

`domain::fechamentos` trava (`status = 'fechado'`) os lancamentos do dia e grava um
resumo (`fechamentos`, hash SHA-256 encadeado sobre os hashes dos movimentos daquele
dia). Nao existe geracao de arquivo PDF pelo backend: a tela de fechamento
(`src/components/FechamentoImpressao.tsx`) e sempre renderizada ao vivo a partir dos
movimentos (ja garantidos imutaveis pelo fechamento) e impressa com `window.print()` do
proprio webview, usando `@media print` para o layout A4. Isso foi uma escolha
deliberada: uma lib de geracao de PDF em Rust (`genpdf`, que usa `printpdf`) traz cerca
de 30 dependencias transitivas, algumas antigas (`time` 0.2, `stdweb`), o oposto do que
se busca com "menor superficie de ataque". Como toda impressora e a maioria dos sistemas
operacionais ja oferecem "salvar como PDF" no proprio dialogo de impressao, isso cobre a
necessidade de exportar sem nenhuma dependencia nova.

## Sincronizacao com Turso e confirmacao de recebimento entre armazens

Cada PC (A4 e B2) continua sendo 100% local e funcional offline — a sincronizacao e um
extra oportunista, nunca uma dependencia pra uso diario. `db::sync` envia os lancamentos
novos (`movimentos.sincronizado_em IS NULL`) para uma tabela consolidada
(`movimentos_consolidados`) num banco [Turso](https://turso.tech) (SQLite gerenciado na
nuvem, free tier), chaveada por `(armazem_codigo, id_origem)` — um upsert idempotente,
seguro se a rede cair no meio do envio. Isso acontece:
- Automaticamente, em segundo plano, toda vez que o app abre (`.setup()` em `lib.rs`) —
  falha (sem internet, por exemplo) so grava um aviso no log, nunca trava o app.
- Sob demanda, pelo botao "Sincronizar agora" no cabecalho (so gestor), que chama o
  comando `sincronizar_agora`.
- Logo apos confirmar o recebimento de uma transferencia (ver abaixo), pra fechar o
  ciclo sem esperar o proximo sync automatico.

**Configurar numa maquina** (uma vez, por PC): crie uma conta em turso.tech, instale a
CLI (`curl -sSfL https://get.tur.so/install.sh | bash`), rode
`turso db create ecoviva-armazem` e `turso db tokens create ecoviva-armazem`, e grave a
URL (`libsql://...`) e o token em duas linhas no arquivo `turso.txt`, na mesma pasta do
banco local (`<diretorio de dados do usuario>/turso.txt`). Sem esse arquivo, a
sincronizacao e pulada silenciosamente — comportamento identico a nao ter configurado.
**O mesmo arquivo `turso.txt` serve pros dois PCs** (A4 e B2 compartilham o mesmo banco
Turso, so o `armazem_codigo` de cada linha muda).

### Confirmacao de recebimento (transferencias entre A4 e B2)

A tela de Montagem registra a saida de peca/scooter montado do galpao com um destino:
"outro armazem" (padrao) ou "outro destino" (ex.: tecnico externo pra conserto de
bateria/modulo/motor — nesse caso o codigo/serie de cada peca vai no campo Observacao
do item, e "pra quem" no campo `contraparte`, sem precisar de coluna nova). Quando o
destino e o outro armazem, `armazem_destino_id` e preenchido sozinho (so existem dois
armazens, sem pergunta extra) e a linha aparece, depois de sincronizada, na tela do
outro armazem como "aguardando confirmacao" — o conferente de la clica "Confirmar
recebimento" (comando `confirmar_recebimento`) e os itens sao copiados automaticamente,
sem redigitar nada.

**Descoberta durante a implementacao**: `transferencia_origem_id INTEGER REFERENCES
movimentos(id)` (no schema desde o inicio, pensado exatamente pra isso) acabou nao
servindo — e uma FK pra uma linha *na mesma tabela local*, e o envio original vive no
banco de **outro PC**; o id local de um PC nao tem relacao com o id local do outro, e o
FK (com `foreign_keys=ON`) rejeitaria o insert. A coluna fica no schema sem uso (nao foi
removida — pode servir pra um caso same-DB futuro). Em vez dela, `movimentos` ganhou
`recebido_de_armazem_codigo`/`recebido_de_id_origem` (migration `0004_transferencias.sql`,
sem FK), guardando a mesma chave composta `(armazem_codigo, id_origem)` que ja identifica
uma linha em `movimentos_consolidados`.

A tabela remota tambem precisou crescer depois de ja estar em uso com dados reais —
`db::sync::SQL_ALTER_TABELA_REMOTA` roda `ALTER TABLE ... ADD COLUMN` a cada sincronizacao
(erro ignorado de proposito: a unica forma de falhar e a coluna ja existir). A consulta de
"o que esta pendente pra mim" (`buscar_pendentes_recebimento`) exclui tanto o que ja foi
confirmado quanto o que foi estornado do lado de quem enviou (dois `NOT EXISTS`), e
`confirmar_recebimento` busca a transferencia de novo no Turso pela chave — nunca confia
nos itens que o frontend mandar de volta — e confere que ela estava mesmo endereçada ao
armazem de quem esta confirmando antes de aceitar.

`db::sync::ler_config_turso`/`movimentos_pendentes`/`marcar_sincronizado` e a logica pura
de parsing (`linha_para_transferencia`) sao cobertas por teste automatizado. O passo de
rede em si (`enviar_para_turso`, `buscar_pendentes_recebimento`, `buscar_transferencia`)
so pode ser validado de ponta a ponta com uma conta/banco Turso real — foi verificado
manualmente simulando os dois PCs (dois bancos SQLite em memoria separados) contra o
Turso real: envio de B2, confirmacao em A4, e o caso de um envio estornado em B2 nao
aparecer mais como pendente em A4.

## Migrations

Arquivos SQL numerados em `src-tauri/migrations/`, aplicados por `rusqlite_migration` a
cada abertura do banco (`db::abrir`). Para mudar o schema, **nunca edite uma migration ja
existente** — crie um novo arquivo `000N_descricao.sql` com o proximo numero.

## Backup automatico (local e externo) e restauracao

`db::backup::backup_automatico` roda uma vez a cada abertura real do app (dentro do
`.setup()` em `lib.rs`, logo apos `db::abrir`) e grava uma copia do banco em
`<diretorio de dados do usuario>/backups/ecoviva-armazem-<AAAA-MM-DD>.db` — um arquivo
por dia (roda de novo no mesmo dia so sobrescreve), mantendo os ultimos 14 dias. Usa a
Online Backup API do SQLite (`Connection::backup`, feature `backup` do `rusqlite`), que
lida corretamente com o modo WAL — diferente de simplesmente copiar o arquivo `.db` no
sistema de arquivos, que poderia perder escritas ainda so no `-wal`. Falha no backup
**nao impede o app de abrir**, so grava um aviso no log.

**Backup externo (pendrive/HD)**: se existir um arquivo `backup_externo.txt` na pasta de
dados (uma linha, com o caminho de destino — ex.: `D:\EcovivaBackups`), o app tambem
grava a copia do dia la, logo apos o backup local, com a mesma retencao de 14 dias
(`db::backup::backup_externo`). Sem esse arquivo, ou com a unidade desconectada no
momento, esse passo e pulado silenciosamente — o backup local continua acontecendo
normalmente de qualquer forma. Configurar isso e manual (crie o arquivo apontando pro
caminho da unidade removivel no PC).

**Para restaurar um backup** (recuperar de um problema, ou trocar de computador):
1. Feche o app completamente.
2. Localize a pasta de dados (no Windows, normalmente
   `%APPDATA%\com.ecoviva.controlearmazem\`) e a subpasta `backups/` dentro dela (ou a
   pasta configurada em `backup_externo.txt`, se for restaurar a partir do backup
   externo).
3. Antes de sobrescrever qualquer coisa, valide o arquivo escolhido com
   `db::backup::verificar_backup_valido` — confere que as tabelas essenciais existem e
   que `PRAGMA integrity_check` esta limpo, pra nao trocar um banco bom por um
   corrompido.
4. Copie o arquivo do dia desejado por cima de `ecoviva-armazem.db` (no mesmo
   diretorio, um nivel acima de `backups/`).
5. Abra o app normalmente.

Esse ciclo completo (gravar backup, apagar o banco original, copiar o backup por cima,
reabrir) e coberto por um teste de integracao automatizado
(`db::backup::tests::restaura_backup_e_dados_e_cadeia_de_hash_sobrevivem`) que confere
nao so que os dados voltam, mas que a cadeia de hash de auditoria
(`domain::movimentos::verificar_cadeia`) continua intacta depois da restauracao.

## Rodando localmente

```bash
npm install
npm run dev          # tauri dev - abre a janela com hot-reload do frontend
```

```bash
cd src-tauri
cargo test                                          # testes de dominio + integracao
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt                                           # ou --check no CI
```

## Adicionando um novo comando

1. Escreva a funcao de dominio em `domain/*.rs` (com testes no mesmo arquivo, modulo
   `#[cfg(test)] mod tests`).
2. Exponha um wrapper fino em `commands/*.rs` com `#[tauri::command]`.
3. Registre em `tauri::generate_handler![...]` dentro de `lib.rs`.
4. Adicione a chamada tipada em `src/lib/api.ts` e os tipos correspondentes em
   `src/types.ts`.

## CI

`.github/workflows/ci.yml` roda em `ubuntu-latest` e `windows-latest` (o alvo real de
distribuicao e Windows): `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`,
`tsc --noEmit` e o build do frontend. So executa de fato quando o repositorio tiver um
remote no GitHub.
