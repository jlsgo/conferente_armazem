-- Fechamento do dia: trava os lancamentos (viram read-only) e registra um
-- resumo auditavel. Nao guarda um arquivo de PDF - a visao de impressao e
-- sempre renderizada ao vivo a partir dos movimentos daquele dia (que ja
-- estao garantidos imutaveis pelo fechamento).

CREATE TABLE fechamentos (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  armazem_id INTEGER NOT NULL REFERENCES armazens(id),
  fluxo TEXT NOT NULL,
  data TEXT NOT NULL,
  usuario_id INTEGER NOT NULL REFERENCES usuarios(id),
  total_itens INTEGER NOT NULL,
  hash_integridade TEXT NOT NULL,
  criado_em TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE(armazem_id, fluxo, data)
);
