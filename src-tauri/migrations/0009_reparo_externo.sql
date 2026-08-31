ALTER TABLE movimento_itens ADD COLUMN codigo_componente TEXT;
CREATE INDEX idx_movimento_itens_codigo_componente ON movimento_itens(codigo_componente);
