-- Sincronizacao oportunista entre A4 e B2 (consolidacao na nuvem via Turso).
-- NULL = nunca enviado; timestamp = ultima vez que foi enviado com sucesso.
-- So marca o que ja foi confirmado do lado da nuvem - se a rede cair no meio
-- do envio, a linha continua NULL e entra na proxima tentativa.
ALTER TABLE movimentos ADD COLUMN sincronizado_em TEXT;
