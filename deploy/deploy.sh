#!/usr/bin/env bash
# Gerado por deploy/onboard para newera-relay. Executado NA VPS pelo
# GitHub Actions, via SSH pelo tunnel:
#   APP_IMAGE=ghcr.io/... /opt/newera-relay/deploy.sh
#
# Fica na máquina (não no CI) de propósito: o CI só diz QUAL imagem subir.
# Toda a lógica de como trocar o container mora aqui, versionada no repo e
# copiada junto no deploy — assim dá pra rodar à mão numa emergência, sem
# depender do GitHub estar no ar.

set -euo pipefail

DIR=/opt/newera-relay
COMPOSE="docker compose -f $DIR/docker-compose.prod.yml --env-file $DIR/.env"

: "${APP_IMAGE:?defina APP_IMAGE, ex ghcr.io/leandrodaf/3d-new-era-ai:v1.0.0}"

log() { printf '%s | %s\n' "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" "$*"; }

cd "$DIR"

# Guarda a imagem anterior ANTES de trocar: é o que permite o rollback
# abaixo sem precisar consultar o registry (que pode estar fora do ar
# justamente quando você precisa voltar).
PREVIOUS=$(grep -E '^APP_IMAGE=' .env 2>/dev/null | cut -d= -f2- || true)
log "imagem atual: ${PREVIOUS:-nenhuma}"
log "imagem nova:  $APP_IMAGE"

# Baixa ANTES de derrubar qualquer coisa. Se o pull falhar (tag errada,
# registry fora), nada foi tocado e a versão em produção segue no ar.
log "baixando imagem"
docker pull "$APP_IMAGE"

# APP_IMAGE vive no .env pra que um `docker compose up` manual na máquina
# suba a mesma versão que o último deploy — sem isso, um restart à mão
# poderia ressuscitar uma versão antiga.
if grep -qE '^APP_IMAGE=' .env; then
  sed -i "s|^APP_IMAGE=.*|APP_IMAGE=$APP_IMAGE|" .env
else
  printf 'APP_IMAGE=%s\n' "$APP_IMAGE" >> .env
fi

# O serviço de backup é buildado localmente (precisa de rclone, que a
# imagem do Postgres não traz). Rebuilda só se o Dockerfile/script mudou.
log "garantindo imagem de backup"
$COMPOSE build backup

log "aplicando"
# --build: o serviço de backup (e, em algumas apps, outros) é construído
# NA máquina a partir do fonte em deploy/. Sem esta flag o compose reusa
# a imagem existente, e uma mudança no fonte nunca chegaria a rodar — o
# deploy passaria VERDE servindo a versão antiga, que é o pior tipo de
# falha: silenciosa e convincente. Com cache, custa quase nada quando
# nada mudou.
$COMPOSE up -d --remove-orphans --build

# wait_healthy espera o /health responder 200. A imagem é `scratch` e não
# tem shell pra um HEALTHCHECK interno — a verificação é aqui, de fora, no
# mesmo endereço que o cloudflared usa.
#
# Virou função porque o ROLLBACK também precisa dela: antes, reverter era
# um `up -d` seguido de "revertido" no log, sem ninguém conferir se a
# versão anterior tinha realmente voltado. Foi assim que o deploy da
# v1.3.0 deixou produção fora do ar com o CI dizendo apenas "failure" —
# a informação de que o site estava DOWN não existia em lugar nenhum.
wait_healthy() {
  local tries=$1
  for _ in $(seq 1 "$tries"); do
    if [ "$(curl -s -o /dev/null -w '%{http_code}' -m 5 http://127.0.0.1:7979/health || echo 000)" = "200" ]; then
      return 0
    fi
    sleep 6
  done
  return 1
}

log "aguardando /health"
if wait_healthy 20; then
  log "saudável"
  # Só limpa imagem velha DEPOIS de confirmar que a nova está de pé —
  # senão um prune apagaria justamente a imagem pra onde voltar.
  docker image prune -f --filter "until=168h" >/dev/null 2>&1 || true
  log "deploy concluído: $APP_IMAGE"
  exit 0
fi

log "ERRO: não ficou saudável em 2 minutos"
$COMPOSE logs --tail 100 app || true

if [ -z "$PREVIOUS" ] || [ "$PREVIOUS" = "$APP_IMAGE" ]; then
  log "ALERTA: SEM VERSÃO ANTERIOR PRA VOLTAR — PRODUÇÃO ESTÁ FORA DO AR"
  exit 1
fi

log "revertendo para $PREVIOUS"
sed -i "s|^APP_IMAGE=.*|APP_IMAGE=$PREVIOUS|" .env

# APP_IMAGE="$PREVIOUS" na frente do comando é O PONTO destas linhas, e
# custou dois incidentes pra ficar claro: este script RECEBE APP_IMAGE como
# variável de ambiente, e o Docker Compose dá precedência à env do shell
# sobre o --env-file. Reescrever o .env acima, sozinho, não reverte nada —
# o compose continua lendo a imagem NOVA da env exportada e recria o
# container na mesma versão quebrada.
#
# Foi assim nos dois rollbacks de 25/08/2026: o log dizia "revertido" e a
# aplicação seguia na imagem que acabara de falhar. --force-recreate
# também fica, pra convergir quando o container está em Restarting.
APP_IMAGE="$PREVIOUS" $COMPOSE up -d --force-recreate app

# O passo que faltava: confirmar que a versão revertida SUBIU. Um rollback
# que não volta é indistinguível de um que voltou, se ninguém olhar.
log "aguardando /health da versão revertida"
if wait_healthy 10; then
  log "revertido e saudável: $PREVIOUS"
  exit 1
fi

log "ALERTA: O ROLLBACK PARA $PREVIOUS TAMBÉM NÃO SUBIU — PRODUÇÃO ESTÁ FORA DO AR"
$COMPOSE logs --tail 50 app || true
exit 1
