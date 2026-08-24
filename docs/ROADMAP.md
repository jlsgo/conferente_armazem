# Roadmap

Registro do que já está pronto e do que vem a seguir, para retomar o trabalho sem
precisar reconstruir o contexto. Atualize esta lista ao concluir/replanejar uma sprint —
não deixe ela ficar desatualizada.

## Feito

- Fundacao Tauri + Rust + SQLite (login, migrations, testes, CI, `docs/ARQUITETURA.md`).
- Fluxo **Saida do Armazem** (veiculos: scooter/triciclo/patinete) completo: lancamento
  de pedido com multiplos itens, lista do dia, total automatico.
- Repositorio no GitHub (`jlsgo/conferente_armazem`), CI verde em Linux e Windows.

## Sprint 1 — Fechar o fluxo principal

O que falta para o fluxo de Saida do Armazem virar o substituto real da planilha:

- **Fechamento do dia**: acao que trava os lancamentos do dia (viram read-only; correcao
  depois so por lancamento de ajuste, nunca edicao direta) e gera um PDF no layout
  parecido com a planilha atual, pronto pra imprimir na impressora do galpao e assinar
  a mao.
- **Cadastro de mais usuarios**: hoje so existe a conta criada na tela de Setup inicial.
  Precisa de uma tela (visivel so para `papel = 'gestor'`) para cadastrar as demais
  conferentes de cada armazem, com login individual.
- Ajustar o icone do instalador para a marca Ecoviva (`PNG/MARCA_ECOVIVA-*.png` ja estao
  no repo).
- **Horario nao reseta sozinho entre lancamentos**: analisando os PDFs antigos, varios
  pedidos seguidos no mesmo dia sao registrados com o mesmo horario (a conferente
  carimba o lote inteiro com o horario em que fechou, nao o horario real de cada
  pedido). O formulario atual reseta o campo Horario para "agora" a cada novo
  lancamento — deveria manter o ultimo valor digitado como padrao do proximo.

## Sprint 2 — Os outros dois fluxos

O schema (`fluxo IN ('peca_montagem', 'sac')`) e o backend generico ja suportam os dois;
falta so a tela:

- **Peca para Montagem (B2 → A4)**: entrada/saida de pecas soltas, com condicao
  (boa/defeito/sucata). Tela parecida com a de Lancamentos, mas para `categoria = 'peca'`
  e sem o campo `numero_pedido` em destaque.
- **SAC**: protocolo + garantia/venda + valor (so quando venda) + itens de peca.

## Sprint 3 — Distribuicao real

- Gerar e testar o instalador Windows de verdade (`.msi`/`.exe` via `cargo tauri build`
  no CI `windows-latest`, ou numa maquina Windows) — ate agora so validamos que o app
  compila e roda no Linux deste ambiente.
- Backup automatico local (copia diaria do arquivo `ecoviva-armazem.db` para outra
  pasta/pendrive) — nenhum backup existe hoje alem do proprio arquivo SQLite.
- Instalar nos PCs reais de A4 e B2 e acompanhar o primeiro uso das conferentes.

## Sprint 4 — Preparar para mais de um armazem "conversarem"

Depende de existir alguma janela de conectividade (ver conversa anterior — ainda nao
confirmado se ha internet em algum ponto do dia):

- Sincronizacao oportunista: quando o PC detectar internet, envia os lancamentos novos
  para um backup/consolidacao central.
- Usar `armazem_destino_id` / `transferencia_origem_id` (ja no schema, sem logica ainda)
  para o check-in de confirmacao entre B2 e A4 que o usuario descreveu: quem libera uma
  peca registra a saida, quem recebe do outro lado confirma a entrada, fechando o ciclo
  e evitando extravio no trajeto.
- Painel consolidado (visao dos dois armazens juntos) para gestao.

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
