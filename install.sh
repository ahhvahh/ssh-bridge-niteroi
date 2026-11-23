#!/bin/sh
set -eu
cd "$(dirname "$0")"
if [ "$(id -u)" -ne 0 ]; then
    echo "Execute sudo ./install.sh" >&2
    exit 1
fi
test -x target/release/ssh-bridge-niteroi
install -d -m 0755 /usr/local/bin
install -d -m 0700 /etc/ssh-bridge-niteroi
install -m 0755 target/release/ssh-bridge-niteroi /usr/local/bin/ssh-bridge-niteroi
if [ ! -e /etc/ssh-bridge-niteroi/fornecer-servicos.yaml ]; then
    install -m 0600 config/fornecer-servicos.example.yaml /etc/ssh-bridge-niteroi/fornecer-servicos.yaml
fi
for grupo in niteroi-ro niteroi-rw niteroi-sockets; do
    getent group "$grupo" >/dev/null || groupadd --system "$grupo"
done
echo "Instalado. Edite /etc/ssh-bridge-niteroi/fornecer-servicos.yaml"
