#!/usr/bin/env bash
# Guarda o developer token do Product Hunt no 1Password sem ele aparecer em
# lugar nenhum: sai da área de transferência e entra no cofre.
#
#   1. em https://www.producthunt.com/v2/oauth/applications, selecione o valor
#      ao lado de "Token:" e copie (⌘C);
#   2. rode: scripts/apoio/guardar-token-ph.sh
#
# O valor não é impresso, não vai para o histórico do shell e não fica no
# repositório. Ao final a área de transferência é limpa e o token é conferido
# contra a API — se ele não responder, o item é apagado de novo.
set -euo pipefail

VAULT="${PH_VAULT:-Agendo Certo}"
TITLE="${PH_ITEM:-Product Hunt - developer token}"
REF="op://${VAULT}/${TITLE}/credential"
raiz="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

command -v op >/dev/null || { echo "o 1Password CLI (op) não está instalado"; exit 1; }

token="$(pbpaste)"
tamanho="${#token}"
if [[ $tamanho -lt 30 || ! "$token" =~ ^[A-Za-z0-9_-]+$ ]]; then
  echo "a área de transferência não tem um token ($tamanho caracteres)."
  echo "copie o valor ao lado de \"Token:\" na página do aplicativo e rode de novo."
  exit 1
fi

if op item get "$TITLE" --vault "$VAULT" >/dev/null 2>&1; then
  op item edit "$TITLE" --vault "$VAULT" "credential[password]=$token" >/dev/null
  echo "token atualizado em $VAULT › $TITLE"
else
  op item create --category "API Credential" --title "$TITLE" --vault "$VAULT" \
    --url "https://www.producthunt.com/v2/oauth/applications" \
    "credential[password]=$token" >/dev/null
  echo "token guardado em $VAULT › $TITLE"
fi
unset token
printf '' | pbcopy   # a área de transferência não precisa continuar com isso

# Confere com a própria API: se o token não responde, não serve de nada.
if op run --env-file <(echo "PH_TOKEN=${REF}") -- node "$raiz/scripts/ph-stats.mjs" --whoami; then
  echo
  echo "para acompanhar o lançamento:"
  echo "  op run --env-file <(echo 'PH_TOKEN=${REF}') -- scripts/ph-stats.mjs --slug 3d-new-era-ai --watch"
else
  echo "o token guardado não foi aceito pela API — confira se copiou o valor certo e rode de novo."
  exit 1
fi
