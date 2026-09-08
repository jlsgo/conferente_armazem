# Plano de melhorias

## Objetivo

Substituir as planilhas de controle dos armazens A4 e B2 por um sistema local,
auditavel e operacionalmente equivalente, sem perder informacoes usadas pelas
conferentes.

## Diagnostico da substituicao

### Conclusao executiva

**Ainda nao e possivel substituir 100% das planilhas pelo sistema novo.**

O fluxo de **Saida de Armazem** esta funcional e cobre o nucleo do registro:
data, horario, pedido, coleta, quantidade, categoria, descricao, quem retirou,
responsavel pelo registro, entrada/saida e fechamento diario.

A substituicao desse fluxo e **parcialmente viavel hoje**, mas ainda faltam
observacoes completas na tela, validacoes de autorizacao no backend e um processo
operacional de correcao/estorno apos o fechamento.

Os fluxos abaixo ainda impedem a substituicao total:

- **Pecas para Montagem:** existe suporte generico no backend, mas nao existe tela.
- **SAC:** o schema possui motivo e valor, mas nao existe tela nem apresentacao
  especifica para protocolo, garantia/venda e pecas.
- **Historico:** os arquivos disponiveis em `modelos_antigos/` sao PDFs. Eles podem
  ser arquivados como referencia, mas nao devem ser importados automaticamente sem
  revisao humana; o layout quebra colunas e ha registros inconsistentes.
- **Integracao A4/B2:** nao existe sincronizacao ou confirmacao de recebimento entre
  computadores.

## Comparacao com os modelos antigos

### 1. Saida de Armazem

Os PDFs de agosto mostram estes campos recorrentes:

- data;
- responsavel, inclusive mais de uma pessoa no mesmo formulario;
- turno;
- numero sequencial da linha;
- horario;
- numero do pedido;
- coleta, transportadora ou cliente;
- quantidade;
- descricao dos veiculos e, em alguns casos, observacoes;
- quem retirou;
- situacao, principalmente `BAIXA`;
- total de unidades do dia.

O sistema novo cobre a maior parte desses campos. O numero da linha e o total sao
calculados automaticamente, e o responsavel passa a ser rastreado por usuario.

**Lacunas para paridade:**

- o formulario novo nao captura `observacoes` do movimento nem observacao por item;
- a descricao livre substitui o texto da coluna de observacoes, mas nao e
  semanticamente a mesma coisa;
- o cabecalho com varios responsaveis e substituido por um usuario por registro;
- a tela nao mostra todos os campos persistidos no banco, como codigo de rastreio;
- nao existe correcao ou estorno formal depois do fechamento.

### 2. Saida B2 - Montagem - Pecas para Armazem

O exemplo contem:

- data e turno;
- horario;
- responsavel pela movimentacao, como `GESON`, `LUCAS` e `BERG - BRUNO`;
- situacao/direcao, como entrada ou saida do galpao B2;
- quantidade de pecas;
- descricao detalhada da peca, inclusive combinacoes de varios itens;
- indicacao de defeito ou sucata;
- total do dia.

O backend ja oferece `entrada`/`saida`, categoria `peca`, descricao, condicao,
quantidade e usuario. Entretanto, sem uma tela dedicada o processo nao pode ser
operado no sistema novo.

**Necessario:** criar tela de montagem, permitir observacao detalhada, validar
condicao (`boa`, `defeito`, `sucata`), exibir claramente origem/destino e testar o
fechamento especifico desse fluxo.

### 3. Controle de Saidas do SAC

O exemplo do SAC contem:

- data e turno;
- responsavel;
- horario;
- numero do protocolo;
- coleta por Correios ou cliente;
- tipo `GARANTIA` ou `VENDA`;
- valor da venda quando aplicavel;
- quantidade de pecas;
- descricao detalhada das pecas e observacoes;
- total de pecas do dia.

O schema ja tem campos que podem representar parte disso: `numero_pedido`,
`contraparte`, `motivo`, `valor_centavos`, categoria `peca` e quantidade. A
semantica ainda precisa ser formalizada: protocolo nao deve aparecer como
numero de pedido, e garantia/venda deve ser um campo controlado.

**Necessario:** criar tela SAC, trocar o destaque para protocolo, exigir motivo,
exigir valor somente para venda, registrar pecas detalhadas e imprimir um resumo
com total de pecas.

## Prioridades

### P0 - Corrigir antes de producao

1. **Autorizacao no backend**
   - Validar usuario existente e ativo ao criar movimento.
   - Garantir que o armazem do movimento corresponde ao armazem do usuario,
     quando o usuario possuir armazem definido.
   - Validar permissao para fechar o dia, preferencialmente somente gestor.
   - Nao confiar em `usuario_id` enviado pela interface como se fosse uma sessao.
   - Adicionar testes tentando operar com usuario de outro armazem, usuario inativo
     e usuario inexistente.

2. **Integridade dos dados**
   - Validar datas reais e horarios entre `00:00` e `23:59`.
   - Validar turno, montagem, condicao e regras por fluxo.
   - Validar que armazens e usuarios relacionados existem e estao ativos.
   - Definir limites para textos e quantidades para evitar registros acidentais.

3. **Auditoria confiavel**
   - Incluir no hash todos os campos relevantes ou documentar formalmente quais sao
     imutaveis e protegidos.
   - Incluir montagem, condicao, observacoes, contraparte, retirante, motivo,
     valor, rastreio e destino quando presentes.
   - Criar rotina de verificacao da cadeia e teste que detecte alteracao de campo.

4. **Correcao apos fechamento**
   - Implementar estorno/ajuste append-only, sem editar o registro original.
   - Exigir justificativa e usuario autorizado.
   - Fazer o fechamento considerar os ajustes posteriores de forma auditavel.

### P1 - Atingir paridade com as planilhas

5. **Finalizar fluxo de Pecas para Montagem**
   - Tela dedicada para B2.
   - Entrada/saida claramente identificada.
   - Pessoa relacionada a movimentacao.
   - Categoria peca, descricao, condicao, quantidade e observacao.
   - Lista diaria, total e fechamento para esse fluxo.

6. **Finalizar fluxo SAC**
   - Protocolo, horario, coleta, garantia/venda e valor.
   - Valor obrigatorio e maior que zero somente em venda.
   - Itens de pecas com descricao e quantidade.
   - Impressao especifica do SAC.

7. **Completar Saida de Armazem**
   - Adicionar observacoes do movimento e do item na tela.
   - Exibir os campos necessarios no fechamento impresso.
   - Confirmar se `montagem` e obrigatorio para veiculos ou apenas opcional.
   - Tratar responsavel por registro e responsaveis do lote sem perder rastreabilidade.

### P2 - Operacao segura

8. **Erros e recuperacao no frontend**
   - Exibir erro de inicializacao em vez de deixar a tela eternamente carregando.
   - Tratar falhas de carregamento da lista e das sugestoes.
   - Desabilitar acoes durante requisicoes e evitar envio duplicado.

9. **Backup e distribuicao**
   - Backup automatico do banco, com retencao e teste de restauracao.
   - Definir procedimento para troca de computador.
   - Gerar e testar instalador Windows em maquina real.
   - Registrar versao do schema e resultado do backup.

10. **Importacao de historico**
    - Solicitar os arquivos originais XLSX/ODS, se existirem.
    - Definir mapeamento de colunas por tipo de planilha.
    - Importar somente apos validacao humana dos totais.
    - Manter os PDFs originais como evidencia, sem sobrescreve-los.

### P3 - Evolucao entre armazens

11. **Sincronizacao oportunista**
    - Enviar eventos quando houver conectividade, sem bloquear o uso offline.
    - Resolver duplicidade e conflito por identificador global.
    - Manter trilha de envio e erro de sincronizacao.

12. **Confirmacao B2 -> A4**
    - Registrar saida na origem.
    - Confirmar entrada no destino.
    - Relacionar os dois movimentos por `transferencia_origem_id`.
    - Exibir pendencias de transporte para a gestao.

## Arquitetura escolhida para varios armazens

Como os armazens nao compartilham a mesma rede Wi-Fi, a solucao recomendada e
uma arquitetura hibrida com servicos online:

```text
Laptop A4: app Tauri + SQLite local --\
                                      API online + banco central
Laptop B2: app Tauri + SQLite local --/
```

- cada laptop continua registrando operacoes localmente, inclusive sem internet;
- o app mantem uma fila de eventos ainda nao sincronizados;
- a API recebe eventos por HTTPS e deve aceitar reenvio sem duplicar registros;
- o banco central consolida os armazens e alimenta um painel web de gestao;
- o destino sincroniza transferencias pendentes e confirma o recebimento;
- a confirmacao retorna ao armazem de origem na proxima sincronizacao;
- cada transferencia e evento deve ter UUID global, nunca apenas o ID local do
  SQLite.

Para manter o custo inicial proximo de zero, o frontend web e a API podem comecar
em planos gratuitos, sujeitos a limites e mudancas do provedor. Antes de usar em
producao, devem ser definidos backup, retencao, autenticacao, limites de uso e um
plano para migrar para um servico pago caso o volume aumente.

O frontend online nao acessara diretamente os arquivos SQLite. A sincronizacao
sera executada pelo app Tauri/Rust de cada laptop, que envia e recebe somente os
eventos autorizados pela API.

## Criterios para declarar substituicao total

A migracao pode ser considerada concluida somente quando:

- os tres tipos de formulario dos PDFs tiverem telas dedicadas;
- todos os campos usados na operacao puderem ser registrados e consultados;
- os totais impressos coincidirem com os totais calculados pelo sistema;
- um gestor puder corrigir um erro por estorno, sem editar ou apagar o original;
- regras de usuario, armazem e fechamento forem verificadas no backend;
- a cadeia de auditoria puder ser verificada;
- houver backup testado e procedimento de restauracao;
- as conferentes validarem pelo menos uma semana de uso paralelo;
- os PDFs antigos permanecerem arquivados e, se necessario, os dados historicos
  forem importados com conferencia dos totais.

## Sequencia recomendada de entrega

1. Autorizacao, validacoes, auditoria e testes de seguranca.
2. Tela de Pecas para Montagem.
3. Tela de SAC.
4. Observacoes, impressao e estorno na Saida de Armazem.
5. Backup, instalador Windows e piloto com uso paralelo.
6. Importacao de historico e sincronizacao entre armazens.
7. API online, fila de sincronizacao e painel web consolidado.

## Decisao atual

O sistema novo **pode substituir imediatamente o papel da Saida de Armazem em um
piloto controlado**, desde que os registros ainda sejam conferidos contra a planilha.
Ele **nao deve ser declarado substituto 100%** antes das telas de Montagem e SAC,
das correcoes pos-fechamento, do backup e das validacoes de autorizacao no backend.

A comunicacao entre armazens sera implementada pela API online, pois eles estao
em redes diferentes. A operacao local nao deve depender de uma conexao permanente:
internet indisponivel significa apenas que eventos ficam pendentes para envio.


