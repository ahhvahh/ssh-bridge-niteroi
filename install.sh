#!/bin/sh
set -eu
cd "$(dirname "$0")"
case "${1-}" in
    ""|--client)
        if [ "$(id -u)" -eq 0 ]; then
            echo "Execute ./install.sh como usuário normal; o script solicita sudo após compilar." >&2
            exit 1
        fi
        if ! command -v cargo >/dev/null 2>&1; then
            echo "Instale Rust/Cargo antes de continuar." >&2
            exit 1
        fi
        cargo build --release
        if [ "${1-}" = --client ]; then
            exec sudo -- ./install.sh --install-client-only
        fi
        exec sudo -- ./install.sh --install-only
        ;;
    --install-only|--install-client-only)
        if [ "$(id -u)" -ne 0 ]; then
            echo "A instalação requer sudo." >&2
            exit 1
        fi
        ;;
    *) echo "Uso: ./install.sh [--client]" >&2; exit 2 ;;
esac
test -x target/release/ssh-bridge-niteroi
if [ "${1-}" = --install-client-only ]; then
    install -d -m 0755 /usr/local/bin
    install -m 0755 target/release/ssh-bridge-niteroi /usr/local/bin/ssh-bridge-niteroi
    echo 'Cliente instalado. Execute ssh-bridge-niteroi client init --server SERVIDOR.'
    exit 0
fi
if [ ! -d /run/systemd/system ]; then
    echo "Esta instalação precisa de systemd em execução." >&2
    exit 1
fi
install -d -m 0755 /usr/local/bin
install -d -m 0700 /etc/ssh-bridge-niteroi
install -m 0755 target/release/ssh-bridge-niteroi /usr/local/bin/ssh-bridge-niteroi
if [ ! -e /etc/ssh-bridge-niteroi/fornecer-servicos.yaml ]; then
    install -m 0600 config/fornecer-servicos.example.yaml /etc/ssh-bridge-niteroi/fornecer-servicos.yaml
fi
for grupo in niteroi-ro niteroi-rw niteroi-sockets niteroi-catalog; do
    getent group "$grupo" >/dev/null || groupadd --system "$grupo"
done
install -m 0644 systemd/niteroi-check.service /etc/systemd/system/niteroi-check.service
install -m 0644 systemd/niteroi-check.timer /etc/systemd/system/niteroi-check.timer
install -d -m 0755 /usr/local/libexec/ssh-bridge-niteroi
install -m 0755 scripts/setup-server.sh /usr/local/libexec/ssh-bridge-niteroi/setup-server
install -m 0644 systemd/niteroi-catalog.socket /etc/systemd/system/niteroi-catalog.socket
install -m 0644 systemd/niteroi-catalog@.service /etc/systemd/system/niteroi-catalog@.service
systemctl daemon-reload
/usr/local/bin/ssh-bridge-niteroi boot-sync
echo "Instalado. Edite /etc/ssh-bridge-niteroi/fornecer-servicos.yaml"
echo "Depois de alterar iniciar_no_boot, execute sudo ssh-bridge-niteroi boot-sync."
