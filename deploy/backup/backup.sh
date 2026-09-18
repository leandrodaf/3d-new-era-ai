#!/bin/bash
# Backup do Postgres para o Cloudflare R2.
#
# Roda como processo de longa duração (não cron): dorme até a hora
# configurada, faz o backup, dorme de novo. Sem cron dentro de container o
# ciclo de vida fica com o Docker — `restart: unless-stopped` cuida de
# reiniciar se cair, e `docker logs backup` mostra tudo.
#
# Três coisas que separam isto de teatro de backup:
#   1. RETENÇÃO em camadas (diário/semanal/mensal), senão você acumula pra
#      sempre ou sobrescreve a única cópia boa.
#   2. VERIFICAÇÃO: todo dump é testado na integridade do gzip e no conteúdo;
#      uma vez por semana é RESTAURADO de verdade num banco descartável e as
#      linhas são contadas. Backup nunca restaurado não é backup.
#   3. HEARTBEAT: um ping externo a cada sucesso. Sem isso, o backup pode
#      parar de rodar e ninguém descobre até precisar dele — o modo de falha
#      mais comum e mais caro que existe nesse assunto.

set -euo pipefail

log() { printf '%s | %s\n' "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" "$*"; }
# Só loga. Quem chama TEM que abortar explicitamente com `|| { fail ...;
# return 1; }` — um `return` aqui dentro sairia só de fail(), não da função
# que chamou. Esse era o bug: a verificação logava "dump suspeito" e seguia
# enviando o arquivo assim mesmo.
fail() { log "ERRO: $*" >&2; }

: "${PGHOST:?}" "${PGUSER:?}" "${PGPASSWORD:?}" "${PGDATABASE:?}"
: "${R2_ACCOUNT_ID:?}" "${R2_ACCESS_KEY_ID:?}" "${R2_SECRET_ACCESS_KEY:?}" "${R2_BUCKET:?}"

BACKUP_HOUR_UTC="${BACKUP_HOUR_UTC:-06}"
KEEP_DAILY="${KEEP_DAILY:-7}"
KEEP_WEEKLY="${KEEP_WEEKLY:-4}"
KEEP_MONTHLY="${KEEP_MONTHLY:-3}"
HEARTBEAT_URL="${BACKUP_HEARTBEAT_URL:-}"

# rclone configurado 100% por variável de ambiente — nenhum arquivo de
# credencial em disco pra vazar ou esquecer com permissão errada.
#
# RCLONE_CONFIG=/dev/null diz explicitamente "não existe arquivo de config";
# sem isso o rclone imprime um NOTICE procurando um a cada execução. Log de
# backup só deve ter linha que alguém precisa ler.
export RCLONE_CONFIG=/dev/null
export RCLONE_CONFIG_R2_TYPE=s3
export RCLONE_CONFIG_R2_PROVIDER=Cloudflare
export RCLONE_CONFIG_R2_ACCESS_KEY_ID="$R2_ACCESS_KEY_ID"
export RCLONE_CONFIG_R2_SECRET_ACCESS_KEY="$R2_SECRET_ACCESS_KEY"
export RCLONE_CONFIG_R2_ENDPOINT="https://${R2_ACCOUNT_ID}.r2.cloudflarestorage.com"
export RCLONE_CONFIG_R2_NO_CHECK_BUCKET=true
export RCLONE_S3_ACL=private

# Piso de sanidade, não de qualidade: pega só o caso patológico de um
# arquivo praticamente vazio. Um banco recém-criado gera dump legítimo de
# ~2KB, então um piso alto reprovaria backup bom — o sinal forte são as
# checagens estruturais abaixo (marcador de fim, presença de COPY), não o
# tamanho.
MIN_BYTES="${BACKUP_MIN_BYTES:-500}"

# Tentativas dentro da MESMA janela, e o intervalo entre elas.
#
# Sem isto, uma falha qualquer — rede instável, pico de carga, o que for —
# deixava o sistema 24 HORAS sem nova tentativa, porque o loop só acorda na
# próxima janela. Foi o que aconteceu em 27/08/2026: a janela das 06:00
# falhou e a cópia mais recente ficou sendo a do dia anterior, sem que nada
# tentasse de novo. Três tentativas espaçadas cobrem o transitório sem
# martelar o banco.
BACKUP_TENTATIVAS="${BACKUP_TENTATIVAS:-3}"
BACKUP_ESPERA_RETRY="${BACKUP_ESPERA_RETRY:-300}"

# Onde guardar o dump que REPROVOU na verificação.
#
# Antes ele era apagado (`rm -f`) junto com a mensagem de erro, e com ele
# ia embora a única evidência do que deu errado — na investigação de
# 27/08/2026 não foi possível dizer POR QUE o dump saiu sem COPY, porque o
# arquivo não existia mais. Um dump reprovado não serve pra restaurar, mas
# serve pra diagnosticar, e ocupa quilobytes.
QUARENTENA="${BACKUP_QUARENTENA:-/tmp/backup-reprovado}"

# ---------------------------------------------------------------------
# Em qual camada de retenção este backup entra.
# Dia 1 do mês -> mensal. Domingo -> semanal. Resto -> diário.
# ---------------------------------------------------------------------
tier_for_today() {
  local dom dow
  dom=$(date -u +%d)
  dow=$(date -u +%u)   # 1=segunda ... 7=domingo
  if [ "$dom" = "01" ]; then echo monthly
  elif [ "$dow" = "7" ]; then echo weekly
  else echo daily
  fi
}

keep_for_tier() {
  case "$1" in
    monthly) echo "$KEEP_MONTHLY" ;;
    weekly)  echo "$KEEP_WEEKLY" ;;
    *)       echo "$KEEP_DAILY" ;;
  esac
}

# ---------------------------------------------------------------------
# Verificação do dump ANTES de subir. Sobe só o que passa.
# ---------------------------------------------------------------------
verify_dump() {
  local file="$1" size

  gunzip -t "$file" 2>/dev/null || { fail "gzip corrompido"; return 1; }

  size=$(stat -c %s "$file")
  [ "$size" -ge "$MIN_BYTES" ] || { fail "dump suspeito: só $size bytes (mínimo $MIN_BYTES)"; return 1; }

  # pg_dump encerra o arquivo com esta linha. Se o processo morreu no meio
  # (OOM, rede, disco cheio), o marcador não existe — e é a checagem mais
  # confiável de "terminou de verdade", mais que o exit code.
  gunzip -c "$file" | tail -20 | grep -q 'PostgreSQL database dump complete' \
    || { fail "dump incompleto: marcador de fim ausente"; return 1; }

  # Precisa ter pelo menos um COPY/INSERT — um dump só-schema passaria em
  # todas as checagens acima e seria inútil numa restauração.
  #
  # SEM `-q` de propósito: `grep -q` sai na primeira linha que casa e o
  # gunzip que ainda despeja o resto leva SIGPIPE (exit 141). Com `set -o
  # pipefail` lá em cima, esse 141 vira falha do pipeline inteiro — e um
  # dump PERFEITO era reprovado como "sem dados" todo dia 06:00 desde que
  # o banco cresceu e passou do buffer do pipe (~64KB). Sem -q o grep lê
  # o stream até o fim, o gunzip termina limpo e o exit code é o do grep.
  gunzip -c "$file" | grep -E '^(COPY|INSERT INTO)' >/dev/null \
    || { fail "dump sem dados: nenhum COPY/INSERT"; return 1; }

  log "verificação ok ($(numfmt --to=iec "$size"))"
}

# ---------------------------------------------------------------------
# Restauração de verdade, num banco descartável, uma vez por semana.
# É a única prova de que o arquivo serve. Roda no mesmo servidor porque é
# o único disponível — por isso é semanal e fora do horário de pico, e o
# banco temporário é sempre derrubado no fim, inclusive em erro.
# ---------------------------------------------------------------------
verify_restore() {
  local file="$1"
  local db="restore_check_$(date -u +%Y%m%d%H%M%S)"

  log "teste de restauração em '$db'"
  # shellcheck disable=SC2064
  trap "psql -q -d postgres -c 'DROP DATABASE IF EXISTS \"$db\";' >/dev/null 2>&1 || true" RETURN

  psql -q -d postgres -c "CREATE DATABASE \"$db\";" >/dev/null

  if ! gunzip -c "$file" | psql -q -d "$db" -v ON_ERROR_STOP=1 >/dev/null 2>&1; then
    fail "restauração falhou — o backup NÃO serve"; return 1
  fi

  local tables rows
  tables=$(psql -tAX -d "$db" -c \
    "SELECT count(*) FROM information_schema.tables WHERE table_schema NOT IN ('pg_catalog','information_schema');")
  rows=$(psql -tAX -d "$db" -c \
    "SELECT COALESCE(sum(n_live_tup),0) FROM pg_stat_user_tables;")

  [ "$tables" -gt 0 ] || { fail "restaurou sem tabela nenhuma"; return 1; }
  log "restauração ok: $tables tabelas, ~$rows linhas"
}

# ---------------------------------------------------------------------
# Retenção: mantém os N mais recentes de cada camada.
# ---------------------------------------------------------------------
prune_tier() {
  local tier="$1" keep total excess
  keep=$(keep_for_tier "$tier")

  mapfile -t files < <(rclone lsf "r2:${R2_BUCKET}/${tier}/" --files-only 2>/dev/null | sort)
  total=${#files[@]}
  [ "$total" -gt "$keep" ] || { log "retenção $tier: $total/$keep, nada a remover"; return 0; }

  excess=$((total - keep))
  for ((i = 0; i < excess; i++)); do
    log "retenção $tier: removendo ${files[i]}"
    rclone deletefile "r2:${R2_BUCKET}/${tier}/${files[i]}" || log "aviso: falha ao remover ${files[i]}"
  done
}

heartbeat() {
  [ -n "$HEARTBEAT_URL" ] || return 0
  curl -fsS -m 15 --retry 3 "$HEARTBEAT_URL" >/dev/null 2>&1 \
    && log "heartbeat enviado" \
    || log "aviso: heartbeat falhou (o backup em si deu certo)"
}

run_backup() {
  local tier file stamp
  tier=$(tier_for_today)
  stamp=$(date -u +%Y-%m-%dT%H-%M-%SZ)
  file="/tmp/${PGDATABASE}-${stamp}.sql.gz"

  log "iniciando backup (camada: $tier)"

  # --no-owner/--no-privileges: o dump restaura em qualquer cluster, sem
  # exigir que app_role/admin_role já existam lá. Essas roles são criadas
  # pela própria aplicação no boot (postgres.BootstrapRoles).
  # PIPESTATUS: sem checar isso, um pg_dump que falha passa despercebido
  # porque o `gzip` no fim do pipe retorna 0.
  set +e
  pg_dump --no-owner --no-privileges | gzip -9 > "$file"
  local status=("${PIPESTATUS[@]}")
  set -e
  [ "${status[0]}" -eq 0 ] || { rm -f "$file"; fail "pg_dump falhou (exit ${status[0]})"; return 1; }
  [ "${status[1]}" -eq 0 ] || { rm -f "$file"; fail "gzip falhou (exit ${status[1]})"; return 1; }

  verify_dump "$file" || { quarentena "$file"; return 1; }

  # Restauração completa só aos domingos: é cara e concorre com a carga do
  # servidor. Nos outros dias vale a verificação estrutural acima.
  if [ "$tier" != "daily" ]; then
    verify_restore "$file" || { quarentena "$file"; return 1; }
  fi

  rclone copyto "$file" "r2:${R2_BUCKET}/${tier}/$(basename "$file")" \
    || { quarentena "$file"; fail "upload pro R2 falhou"; return 1; }
  log "enviado: ${tier}/$(basename "$file")"

  rm -f "$file"
  prune_tier "$tier"
  heartbeat
  log "backup concluído"
}

# quarentena guarda o dump reprovado em vez de apagá-lo, mantendo só os
# 3 mais recentes — evidência serve pra diagnóstico, não pra acumular.
quarentena() {
  local file="$1"
  [ -f "$file" ] || return 0
  mkdir -p "$QUARENTENA"
  mv "$file" "$QUARENTENA/" 2>/dev/null || { rm -f "$file"; return 0; }
  log "dump reprovado guardado em $QUARENTENA/$(basename "$file") — inspecione com zcat"

  # ls -t: mais recente primeiro; tail -n +4 são o 4º em diante.
  local velhos
  velhos=$(ls -t "$QUARENTENA"/*.sql.gz 2>/dev/null | tail -n +4 || true)
  [ -n "$velhos" ] && printf '%s\n' "$velhos" | xargs -r rm -f
  return 0
}

# run_backup_com_retry insiste dentro da mesma janela.
#
# Um `return 0` na primeira tentativa que der certo: as tentativas
# seguintes só existem pro caso de falha, e repetir um backup que já
# funcionou seria desperdício de I/O no banco de produção.
run_backup_com_retry() {
  local i
  for ((i = 1; i <= BACKUP_TENTATIVAS; i++)); do
    if run_backup; then
      [ "$i" -gt 1 ] && log "sucesso na tentativa $i de $BACKUP_TENTATIVAS"
      return 0
    fi
    if [ "$i" -lt "$BACKUP_TENTATIVAS" ]; then
      log "tentativa $i de $BACKUP_TENTATIVAS falhou; nova tentativa em ${BACKUP_ESPERA_RETRY}s"
      sleep "$BACKUP_ESPERA_RETRY"
    fi
  done
  fail "as $BACKUP_TENTATIVAS tentativas desta janela falharam"
  return 1
}

# ---------------------------------------------------------------------
# Loop principal: dorme até BACKUP_HOUR_UTC e roda uma vez por dia.
# ---------------------------------------------------------------------
log "backup iniciado — alvo diário às ${BACKUP_HOUR_UTC}:00 UTC, bucket r2:${R2_BUCKET}"
log "retenção: ${KEEP_DAILY} diários / ${KEEP_WEEKLY} semanais / ${KEEP_MONTHLY} mensais"

if [ "${BACKUP_ON_START:-false}" = "true" ]; then
  log "BACKUP_ON_START=true — rodando agora"
  run_backup_com_retry || log "backup inicial falhou; seguindo para o ciclo normal"
fi

while true; do
  now_h=$(date -u +%H); now_m=$(date -u +%M)
  # Segundos até a próxima ocorrência de BACKUP_HOUR_UTC:00.
  target=$(( (10#$BACKUP_HOUR_UTC * 3600) - (10#$now_h * 3600 + 10#$now_m * 60) ))
  [ "$target" -le 0 ] && target=$((target + 86400))
  log "dormindo ${target}s até a próxima janela"
  sleep "$target"

  # `|| log` de propósito: uma falha não pode matar o loop, senão um erro
  # transitório de rede desliga o backup pra sempre. O heartbeat que não
  # chega é o que denuncia a falha — e é por isso que ele existe.
  # O `|| log` continua de propósito: uma falha não pode matar o loop,
  # senão um erro transitório desligaria o backup pra sempre. A diferença
  # é que agora já houve BACKUP_TENTATIVAS tentativas antes de desistir
  # da janela.
  run_backup_com_retry || log "ERRO: todas as tentativas desta janela falharam; próxima janela em 24h"
done
