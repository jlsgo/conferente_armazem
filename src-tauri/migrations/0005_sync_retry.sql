ALTER TABLE movimentos ADD COLUMN sync_tentativas INTEGER NOT NULL DEFAULT 0;
ALTER TABLE movimentos ADD COLUMN sync_erro TEXT;
ALTER TABLE movimentos ADD COLUMN sync_proxima_tentativa TEXT;
