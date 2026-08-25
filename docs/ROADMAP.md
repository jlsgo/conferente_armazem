# Roadmap

Registro do que já está pronto e do que vem a seguir, para retomar o trabalho sem
precisar reconstruir o contexto. Atualize esta lista ao concluir/replanejar uma sprint —
não deixe ela ficar desatualizada.

## Feito

- Fundacao Tauri + Rust + SQLite (login, migrations, testes, CI, `docs/ARQUITETURA.md`).
- Fluxo **Saida do Armazem** (veiculos: scooter/triciclo/patinete) completo: lancamento
  de pedido com multiplos itens, lista do dia, total automatico.
- Repositorio no GitHub (`jlsgo/conferente_armazem`), CI verde em Linux e Windows.
- **Sprint 1**: fechamento do dia (trava os lancamentos e gera uma visao de impressao em
  A4 via `window.print()` do proprio app, sem depender de nenhuma lib de PDF — ver
  `docs/ARQUITETURA.md`); cadastro de mais usuarios (tela restrita a `papel = 'gestor'`,
  checado no backend); icone do instalador trocado pela marca Ecoviva; horario do
  lancamento nao reseta mais sozinho entre pedidos do mesmo lote.
- **Sprint 2**: sessao real no backend (`AppState.sessao`) — `login`/`logout` passam a
  controlar quem esta autenticado; `criar_movimento`, `fechar_dia` e `criar_usuario`
  nao aceitam mais `usuario_id`/`solicitante_id` vindo do payload, usam a sessao.
  Validacao de turno/montagem/condicao, limite de texto (500 caracteres) e quantidade
  (100.000), e checagem de usuario/armazem ativos e correspondentes em toda escrita.
  Hash de auditoria (`calcular_hash`) agora cobre todos os campos do movimento (nao so
  uma fracao) e ha uma rotina `verificar_cadeia` que detecta alteracao direta no banco
  (com teste). Estorno append-only (`estornar_movimento`, usa a coluna `estornado_de`
  que ja existia no schema) — corrige um lancamento mesmo com o dia fechado, exige
  gestor + justificativa, e o fechamento mostra `total_estornado`/`total_liquido`
  calculados na hora (sem editar o registro do fechamento). UI: botao "Estornar" para
  gestores em `Lancamentos.tsx`, botao "Fechar o dia" some para conferentes.
- **Sprint 3**: telas de `Montagem.tsx` (fluxo `peca_montagem`: entrada/saida de peca
  solta no galpao B2, condicao boa/defeito/sucata obrigatoria por item, sem
  `numero_pedido`) e `Sac.tsx` (fluxo `sac`: protocolo, coleta, garantia/venda com
  valor obrigatorio so quando venda, `tipo` fixo em `entrada` sem toggle pra reduzir
  campo pro conferente preencher). Validacao das duas regras no backend
  (`domain::movimentos::validar_novo_movimento`), nao so na tela. `Movimento` (Rust e
  TS) passou a expor `motivo`/`valor_centavos`, que faltavam pra tela do SAC.
  `FechamentoImpressao` ganhou uma prop `variante` (`armazem`/`montagem`/`sac`) pra
  imprimir cada fluxo com as colunas certas. Aba de navegacao no `Dashboard.tsx` agora
  aparece pra qualquer usuario logado (antes so gestor via nav).

## Sprint 4 — Completar Saida de Armazem + distribuicao real

- Adicionar `observacoes` do movimento e do item na tela (lacuna apontada no plano de
  melhorias: hoje a descricao livre nao substitui a coluna de observacoes das planilhas).
- Exibir no fechamento impresso os campos que ja existem no banco mas nao aparecem na
  tela hoje (ex.: codigo de rastreio).
- Gerar e testar o instalador Windows de verdade (`.msi`/`.exe` via `cargo tauri build`
  no CI `windows-latest`, ou numa maquina Windows) — ate agora so validamos que o app
  compila e roda no Linux deste ambiente.
- Backup automatico local (copia diaria do arquivo `ecoviva-armazem.db` para outra
  pasta/pendrive) — nenhum backup existe hoje alem do proprio arquivo SQLite.
- Instalar nos PCs reais de A4 e B2 e acompanhar o primeiro uso das conferentes (piloto
  em paralelo com a planilha, conforme a "Decisao atual" do plano de melhorias).

## Sprint 5 — Preparar para mais de um armazem "conversarem"

Depende de existir alguma janela de conectividade (ver conversa anterior — ainda nao
confirmado se ha internet em algum ponto do dia):

- Sincronizacao oportunista: quando o PC detectar internet, envia os lancamentos novos
  para um backup/consolidacao central.
- Usar `armazem_destino_id` / `transferencia_origem_id` (ja no schema, sem logica ainda)
  para o check-in de confirmacao entre B2 e A4 que o usuario descreveu: quem libera uma
  peca registra a saida, quem recebe do outro lado confirma a entrada, fechando o ciclo
  e evitando extravio no trajeto.
- Painel consolidado (visao dos dois armazens juntos) para gestao.
- **Importacao de historico**: pedir os XLSX/ODS originais se existirem (os PDFs em
  `modelos_antigos/` quebram coluna e tem registros inconsistentes, entao ficam so como
  arquivo de referencia); definir mapeamento de colunas por tipo de planilha; importar
  somente depois de validacao humana dos totais.

## Depois disso

- Relatorios e exportacao para Excel.
- Escalar o mesmo instalador para novos armazens, se a empresa abrir mais.

## Decisoes que ja foram tomadas (nao reabrir sem motivo novo)

- Sem controle de saldo de estoque — e um livro de movimentacao/auditoria, nao um
  sistema de estoque.
- Sem catalogo de produto fixo — categoria (lista curta) + descricao livre com
  autocomplete.
- Uma unica `Mutex<Connection>` no backend, nao um pool.
- App 100% local por PC; qualquer sincronizacao e oportunista, nunca uma dependencia
  para o uso diario.
- Sem biblioteca de geracao de PDF em Rust — o fechamento do dia e impresso direto do
  app via `window.print()` (CSS `@media print`, tamanho A4), evitando dependencias
  transitivas desnecessarias (uma lib como `genpdf`/`printpdf` traria ~30 crates extras,
  varios antigos).
