# Plano de melhorias futuras

## Objetivo

Avaliar e preparar a possibilidade de o administrador acompanhar, em tempo quase
real, o que cada conferente registra em cada armazem, sem retirar o funcionamento
offline dos laptops.

## Conclusao inicial

A funcionalidade e tecnicamente possivel, mas ainda nao existe no sistema atual.
Hoje cada computador possui seu proprio SQLite local. Portanto, um administrador
nao consegue acompanhar remotamente os registros de outro armazem enquanto eles
nao forem sincronizados.

A solucao futura recomendada e:

```text
Laptop de cada armazem
  app Tauri + SQLite local
           |
           | HTTPS, quando houver internet
           v
API online + banco central
           |
           v
Painel web do administrador
```

O registro deve continuar sendo salvo primeiro no SQLite local. A internet deve
servir para sincronizar e acompanhar, nao para bloquear a operacao do armazem.

## O que o administrador devera visualizar

- registros mais recentes de todos os armazens;
- armazem de origem;
- nome da conferente;
- data e horario do registro;
- tipo de movimento: entrada ou saida;
- fluxo: armazem, montagem ou SAC;
- pedido ou protocolo;
- itens, descricoes e quantidades;
- situacao do movimento;
- horario do ultimo envio para a nuvem;
- registros ainda pendentes de sincronizacao;
- laptops sem comunicacao recente;
- transferencias em transito;
- divergencias entre quantidade enviada e recebida.

## Niveis de atualizacao

### Nivel 1 - Sincronizacao periodica

O laptop envia novos registros a cada poucos segundos ou ao terminar uma
operacao. O painel consulta a API em intervalos de 5 a 15 segundos.

**Vantagens:** menor complexidade e custo inicial.

**Resultado esperado:** acompanhamento quase em tempo real, normalmente com
atraso de alguns segundos.

### Nivel 2 - Atualizacao por eventos

A API informa o painel imediatamente quando recebe um novo registro, usando
WebSocket ou Server-Sent Events.

**Vantagens:** atualizacao mais rapida e menos consultas repetidas.

**Pre-requisito:** a sincronizacao dos laptops e a API ja devem estar estaveis.

### Nivel 3 - Operacao multi-armazem completa

Alem de visualizar registros, o administrador acompanha transferencias entre
armazens, recebimentos, divergencias, usuarios online e fila de sincronizacao.

Esse nivel depende de o fluxo de transferencia B2 -> A4 estar implementado.

## Etapas de implementacao

### Fase 0 - Decisao e levantamento

- confirmar quantos armazens e usuarios serao atendidos;
- confirmar se os laptops possuem internet durante o expediente;
- estimar quantidade de movimentos por dia;
- definir quais dados o administrador pode consultar;
- definir prazo aceitavel para um registro aparecer no painel;
- confirmar necessidade de acesso por celular ou somente computador;
- escolher entre atualizacao por polling e eventos em tempo real.

**Criterio de saida:** requisitos de conectividade, volume, usuarios e prazo de
atualizacao aprovados.

### Fase 1 - Identificador e fila local

- adicionar UUID global aos movimentos e futuros eventos;
- criar tabela local de eventos de sincronizacao;
- registrar estado: pendente, enviado, confirmado ou erro;
- implementar tentativas com espera progressiva;
- garantir idempotencia para que o mesmo evento nao seja duplicado;
- registrar data da ultima tentativa e mensagem de erro segura;
- testar perda de internet durante um lancamento.

**Criterio de saida:** nenhum registro local e perdido ou duplicado quando a
conexao cai e retorna.

### Fase 2 - API e banco central

- criar API autenticada por HTTPS;
- criar banco central para armazens, usuarios, movimentos e sincronizacoes;
- receber eventos assinados/autorizados pelo app local;
- validar armazem, usuario, UUID e schema do evento;
- rejeitar duplicidade sem rejeitar reenvio legitimo;
- guardar a origem e o horario do evento;
- aplicar autorizacao por papel e armazem;
- criar endpoint de envio e endpoint de busca incremental.

**Criterio de saida:** dois laptops conseguem enviar e consultar seus eventos
sem duplicidade e sem acesso indevido aos dados de outro armazem.

### Fase 3 - Sincronizacao no app Tauri

- sincronizar ao iniciar o app;
- sincronizar apos salvar um movimento;
- repetir automaticamente quando houver internet;
- mostrar ao usuario o estado da sincronizacao;
- permitir consultar a fila com erro;
- baixar eventos destinados ao armazem local;
- nao bloquear lancamentos enquanto a API estiver indisponivel.

**Criterio de saida:** um movimento feito no B2 aparece no servidor e depois no
A4 dentro do prazo definido, sem impedir o uso offline.

### Fase 4 - Painel administrativo

- criar login administrativo separado ou perfil gestor central;
- mostrar registros recentes em ordem cronologica;
- filtrar por armazem, conferente, fluxo, data e situacao;
- mostrar indicador de ultima sincronizacao por laptop;
- destacar movimentos pendentes ou com erro;
- atualizar automaticamente sem recarregar a pagina;
- permitir exportar o resultado sem alterar os dados originais;
- registrar auditoria das consultas e acoes administrativas.

**Criterio de saida:** o administrador consegue localizar um registro de qualquer
armazem e identificar se ele foi sincronizado ou continua apenas local.

### Fase 5 - Transferencias entre armazens

- criar transferencia com origem, destino e UUID global;
- registrar itens e quantidades enviadas;
- gerar codigo ou QR Code da transferencia;
- mostrar status `criada`, `em_transito`, `recebida`, `parcial` ou `divergente`;
- permitir confirmacao no armazem destino;
- registrar quantidade recebida, responsavel e horario;
- preservar a saida original e criar evento separado de recebimento;
- exibir pendencias para o administrador.

**Criterio de saida:** o administrador consegue saber o que saiu, o que foi
recebido e quais caixas ou pecas ainda estao em transito.

## Seguranca e integridade

- nunca expor diretamente o arquivo SQLite ao navegador;
- nunca colocar token da API no frontend publico;
- usar HTTPS em toda comunicacao;
- armazenar tokens somente no app local e em variaveis seguras no servidor;
- separar permissao de conferente, gestor local e administrador central;
- validar no servidor o armazem de cada evento;
- manter movimentos originais imutaveis;
- usar UUID, idempotencia e controle de versao dos eventos;
- registrar logs sem senha, token ou dados desnecessarios;
- criar backup do banco central e dos SQLite locais.

## Custo e escolha tecnica inicial

Para um primeiro piloto, a alternativa mais simples e economica e:

- SQLite local em cada laptop;
- API online pequena;
- Turso ou outro banco central compativel com SQLite;
- polling de 5 a 15 segundos no painel;
- hospedagem gratuita enquanto o volume permitir;
- monitoramento basico de erros e sincronizacao.

WebSocket ou Server-Sent Events podem ser adicionados depois. Nao e necessario
comecar com essa complexidade para validar se o acompanhamento quase em tempo
real resolve a necessidade da gestao.

## Testes obrigatorios

- registrar movimento sem internet;
- reconectar e confirmar envio automatico;
- reenviar o mesmo evento varias vezes;
- desligar o laptop durante o envio;
- receber eventos fora de ordem;
- tentar consultar outro armazem sem permissao;
- testar dois laptops criando o mesmo ID local;
- testar quantidade enviada diferente da recebida;
- restaurar backup local e central;
- verificar atraso real entre registro e exibicao no painel;
- testar volume de um dia completo de operacao;
- verificar que o painel continua somente leitura para movimentos fechados.

## Decisao de viabilidade

A funcionalidade deve ser considerada viavel para um piloto quando:

- houver internet funcional nos armazens durante parte do expediente;
- a fila local suportar indisponibilidade prolongada;
- a API aceitar reenvio sem duplicidade;
- o administrador puder consultar os dados por armazem e conferente;
- o atraso maximo de sincronizacao for conhecido e aceitavel;
- houver backup e procedimento de recuperacao;
- os usuarios validarem o fluxo durante pelo menos uma semana.

## Ordem recomendada

1. Confirmar conectividade, volume e prazo de atualizacao.
2. Implementar UUID global e fila de sincronizacao.
3. Criar API online e banco central.
4. Sincronizar os movimentos atuais.
5. Criar painel administrativo somente leitura.
6. Testar polling de 5 a 15 segundos.
7. Adicionar eventos em tempo real se o polling nao for suficiente.
8. Implementar transferencia e confirmacao B2 -> A4.
9. Fazer piloto com planilha paralela e monitorar falhas.

## Resultado esperado

Ao final, o administrador podera acompanhar os registros dos armazens quase em
tempo real, sabendo a diferenca entre:

```text
Registrado localmente
Sincronizado com a API
Visualizado no painel
Recebido no armazem destino
```

A internet indisponivel nao apagara o registro nem impedira a conferente de
trabalhar; apenas atrasara sua aparicao no painel administrativo.
