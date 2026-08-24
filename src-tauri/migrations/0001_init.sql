-- Ecoviva - Controle de Armazens
-- Banco local (SQLite), um arquivo por PC/armazem. Offline-first.

CREATE TABLE armazens (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  codigo TEXT NOT NULL UNIQUE,        -- 'A4', 'B2'
  nome TEXT NOT NULL,
  ativo INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE usuarios (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  nome TEXT NOT NULL,
  login TEXT NOT NULL UNIQUE,
  senha_hash TEXT NOT NULL,            -- string PHC completa (argon2)
  armazem_id INTEGER REFERENCES armazens(id),
  papel TEXT NOT NULL DEFAULT 'conferente',   -- 'conferente' | 'gestor'
  ativo INTEGER NOT NULL DEFAULT 1,
  criado_em TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Um "pedido"/"protocolo" pode ter varios itens dentro dele.
-- fluxo: 'saida_armazem' (veiculos), 'peca_montagem' (B2 -> A4), 'sac' (garantia/venda).
CREATE TABLE movimentos (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  armazem_id INTEGER NOT NULL REFERENCES armazens(id),
  armazem_destino_id INTEGER REFERENCES armazens(id),   -- so em transferencias entre armazens
  fluxo TEXT NOT NULL,
  tipo TEXT NOT NULL,                  -- 'entrada' | 'saida'
  data TEXT NOT NULL,                  -- 'YYYY-MM-DD'
  hora TEXT NOT NULL,                  -- 'HH:MM'
  turno TEXT NOT NULL DEFAULT 'diurno',
  usuario_id INTEGER NOT NULL REFERENCES usuarios(id),
  numero_pedido TEXT,
  codigo_rastreio TEXT,
  contraparte TEXT,
  quem_retirou TEXT,
  motivo TEXT,                         -- 'garantia' | 'venda' (fluxo sac)
  valor_centavos INTEGER,
  observacoes TEXT,
  status TEXT NOT NULL DEFAULT 'aberto',      -- 'aberto' | 'fechado'
  transferencia_origem_id INTEGER REFERENCES movimentos(id), -- liga check-in futuro a saida original
  estornado_de INTEGER REFERENCES movimentos(id),
  hash_integridade TEXT NOT NULL,
  criado_em TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE movimento_itens (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  movimento_id INTEGER NOT NULL REFERENCES movimentos(id) ON DELETE CASCADE,
  categoria TEXT NOT NULL,             -- 'scooter' | 'triciclo' | 'patinete' | 'peca'
  descricao TEXT,                      -- texto livre opcional, ex: 'HE-15 GREEN'
  montagem TEXT,                       -- 'montado' | 'caixa' | NULL (so relevante p/ veiculos)
  condicao TEXT,                       -- 'boa' | 'defeito' | 'sucata' | NULL (so relevante p/ pecas)
  quantidade INTEGER NOT NULL CHECK (quantidade > 0),
  observacao TEXT
);

CREATE INDEX idx_movimentos_dia ON movimentos(armazem_id, fluxo, data);
CREATE INDEX idx_movimento_itens_movimento ON movimento_itens(movimento_id);
CREATE INDEX idx_movimento_itens_categoria_desc ON movimento_itens(categoria, descricao);
