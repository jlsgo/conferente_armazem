-- Razao social / nome fantasia do cliente/transportadora, em saida_armazem
-- e sac - texto livre opcional, complementar ao "contraparte" (coleta, o
-- nome usado no dia a dia, nem sempre o nome legal da empresa).
--
-- Deliberadamente NAO entra no hash de auditoria (calcular_hash em
-- domain/movimentos.rs): incluir um campo novo ali mudaria o formato do
-- hash a partir de agora, quebrando verificar_cadeia para todo lancamento
-- ja gravado antes desta migracao.
ALTER TABLE movimentos ADD COLUMN razao_social TEXT;
