#!/usr/bin/env bash
# Gera painel/local/index.html com as credenciais reais do Turso embutidas,
# a partir do template publico painel/index.html (o mesmo processo que
# .github/workflows/deploy-painel.yml faz no deploy, so que local e sem
# publicar nada). Saida fica em painel/local/, que e git-ignorado.
set -euo pipefail

DIR_RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARQUIVO_CREDENCIAIS="$DIR_RAIZ/painel/local/credenciais.env"
TEMPLATE="$DIR_RAIZ/painel/index.html"
SAIDA="$DIR_RAIZ/painel/local/index.html"

if [[ ! -f "$ARQUIVO_CREDENCIAIS" ]]; then
  echo "Erro: $ARQUIVO_CREDENCIAIS nao existe." >&2
  exit 1
fi

# shellcheck disable=SC1090
source "$ARQUIVO_CREDENCIAIS"

if [[ -z "${TURSO_PAINEL_URL:-}" || -z "${TURSO_PAINEL_TOKEN:-}" ]]; then
  echo "Erro: TURSO_PAINEL_URL ou TURSO_PAINEL_TOKEN nao definidos em $ARQUIVO_CREDENCIAIS." >&2
  exit 1
fi

sed \
  -e "s#__TURSO_PAINEL_URL__#${TURSO_PAINEL_URL}#g" \
  -e "s#__TURSO_PAINEL_TOKEN__#${TURSO_PAINEL_TOKEN}#g" \
  "$TEMPLATE" > "$SAIDA"

echo "Gerado: $SAIDA"
