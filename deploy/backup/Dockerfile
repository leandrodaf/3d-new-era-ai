# Imagem de backup: Postgres client + rclone.
#
# A base é a MESMA imagem/versão do servidor de propósito. pg_dump se recusa
# a rodar contra um servidor mais novo que ele ("server version mismatch"),
# então usar postgres:16 aqui com servidor 17 quebraria o backup — e só se
# descobre no dia em que ele é necessário. Atualizar esta linha JUNTO com a
# do serviço `postgres` no compose.
FROM postgres:17.6-bookworm

# rclone vem do binário OFICIAL, não do apt.
#
# O Debian bookworm empacota rclone 1.60.1, de 2022 — e essa versão manda
# um cabeçalho de checksum que o R2 não implementa: TODO upload falhava com
# "NotImplemented: 501", o rclone repetia e só então passava. O backup
# funcionava, mas gastava dois uploads e cuspia erro vermelho todo dia, que
# é como se aprende a ignorar log.
#
# Medido no container real: 1.60.1 dá 501 em toda tentativa (com ou sem
# --s3-disable-checksum, --ignore-checksum, --s3-upload-cutoff); 1.75.0 sobe
# limpo. Não é flag, é versão.
#
# Versão e checksum fixados: `curl | dpkg -i` sem verificar seria confiar
# cegamente em quem responder aquele DNS na hora do build.
ARG RCLONE_VERSION=v1.75.0
ARG RCLONE_SHA256=266598b5c66a42b821571332013cc85a88ffcff8939cf0643861c8d033dd3a4a

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && curl -fsSL -o /tmp/rclone.deb \
       "https://downloads.rclone.org/${RCLONE_VERSION}/rclone-${RCLONE_VERSION}-linux-amd64.deb" \
    && echo "${RCLONE_SHA256}  /tmp/rclone.deb" | sha256sum -c - \
    && dpkg -i /tmp/rclone.deb \
    && rm -f /tmp/rclone.deb \
    && rm -rf /var/lib/apt/lists/* \
    && rclone version | head -1

COPY backup.sh /opt/backup/backup.sh
RUN chmod +x /opt/backup/backup.sh

# O usuário `postgres` já existe na imagem base (uid 999). Rodar como ele em
# vez de root: o script só precisa ler do banco, escrever em /tmp e falar
# HTTPS — nada disso exige privilégio.
USER postgres

ENTRYPOINT ["/bin/bash", "/opt/backup/backup.sh"]
