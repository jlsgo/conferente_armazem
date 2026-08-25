-- Confirmacao de recebimento de uma transferencia vinda do outro armazem.
-- Nao usa `transferencia_origem_id` (FK pra movimentos.id) porque o envio
-- original vive no banco local de OUTRO PC - o id la nao tem relacao com o
-- id aqui, e o FK com foreign_keys=ON rejeitaria o insert. Guarda em vez
-- disso a chave composta (armazem_codigo, id_origem) que identifica a linha
-- em `movimentos_consolidados` no Turso.
ALTER TABLE movimentos ADD COLUMN recebido_de_armazem_codigo TEXT;
ALTER TABLE movimentos ADD COLUMN recebido_de_id_origem INTEGER;
