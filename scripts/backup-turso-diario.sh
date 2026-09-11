#!/usr/bin/env bash
# Backup diario do banco Turso de producao (ecoviva-armazem) pro laptop local
# do jlsgo - camada extra de seguranca alem da replicacao do proprio Turso e
# do dump/upload offsite que o app ja faz em cada armazem (ver
# src-tauri/src/db/sync.rs, exportar_consolidado + backup_nuvem.rs). Pensado
# pra rodar via cron as 17:50 (fim do expediente).
#
# `turso db export` baixa um snapshot binario direto (nao roda SELECT
# nenhum), entao nao conta pra cota gratuita de "rows read" do Turso - ver
# a regra de gasto zero deste sistema. Precisa do `turso` CLI autenticado
# (`turso auth login`) na conta que roda este cron (mesmo usuario/HOME).
#
# Caminho do binario e hardcoded (em vez de depender de PATH) porque cron
# roda com um PATH minimo, sem o que o .bashrc interativo adiciona.
set -euo pipefail

TURSO_BIN="$HOME/.turso/turso"
BANCO="ecoviva-armazem"
DESTINO="$HOME/backups-turso-ecoviva"
RETENCAO_DIAS=14
DATA="$(date +%F)"

mkdir -p "$DESTINO"

"$TURSO_BIN" db export "$BANCO" \
  --output-file "$DESTINO/$BANCO-$DATA.db" \
  --overwrite \
  --with-metadata

# Mesma politica de retencao dos backups locais do app
# (src-tauri/src/db/backup.rs, RETENCAO_DIAS) - apaga o .db e os arquivos
# irmaos (-wal/-info) mais antigos que a retencao.
find "$DESTINO" -maxdepth 1 -name "$BANCO-*.db*" -mtime "+$RETENCAO_DIAS" -delete

echo "$(date '+%F %T') - backup ok: $DESTINO/$BANCO-$DATA.db"
