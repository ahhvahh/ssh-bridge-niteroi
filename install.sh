#!/bin/sh
set -eu
cd "$(dirname "$0")"
# Assistente interativo. Os modos anteriores continuam disponíveis para automação.
cli=/usr/local/bin/ssh-bridge-niteroi
prompt() {
    printf '%s ' "$1" >&2
    IFS= read -r answer || { echo 'Entrada encerrada. Execute o assistente novamente para continuar.' >&2; exit 1; }
}
case "${1-}" in
    cliente|servidor)
        [ "$#" -eq 1 ] || { echo 'Uso: ./install.sh cliente|servidor' >&2; exit 2; }
        [ "$(id -u)" -ne 0 ] || { echo 'Inicie como usuário normal. O instalador solicitará sudo.' >&2; exit 1; }
        command -v cargo >/dev/null || { echo 'Instale Rust/Cargo primeiro.' >&2; exit 1; }
        if [ "$1" = cliente ]; then
            for cmd in ssh ssh-keygen sftp sshfs fusermount3; do
                command -v "$cmd" >/dev/null || { echo "Dependência ausente: $cmd (instale openssh-client e sshfs)." >&2; exit 1; }
            done
        fi
        cargo build --release
        prompt 'Hostname ou IP do servidor, como será acessado pelo cliente [scb01]:'
        server=${answer:-scb01}
        case "$server" in -*|*[!a-zA-Z0-9.-]*) echo 'Nome de servidor inválido.' >&2; exit 2;; esac
        if [ "$1" = cliente ]; then
            sudo -- ./install.sh --install-client-only
            "$cli" client init --server "$server"
            printf '\nChave PÚBLICA deste cliente para colar no servidor:\n'
            cat "$HOME/.ssh/id_ed25519_niteroi.pub"
            prompt 'Execute ./install.sh servidor no Debian. Após autorizar a chave e obter a impressão digital do servidor, pressione Enter.'
            echo 'No primeiro acesso, confira a impressão digital Ed25519 exibida pelo SSH com a do console do servidor.'
            printf 'quit\n' | sftp -F /dev/null -o HostKeyAlgorithms=ssh-ed25519 -o StrictHostKeyChecking=ask \
                -o PasswordAuthentication=no -o KbdInteractiveAuthentication=no -o IdentitiesOnly=yes \
                -i "$HOME/.ssh/id_ed25519_niteroi" "niteroi@$server"
            "$cli" client list
            echo 'Para montar as pastas: ssh-bridge-niteroi client mount-all'
        else
            sudo -- ./install.sh --install-only
            sudo systemctl enable --now ssh.service
            printf '\nImpressão digital Ed25519 do servidor (confira no cliente):\n'
            ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub
            prompt 'Cole a linha pública ssh-ed25519 exibida no cliente e pressione Enter:'
            umask 077
            pub=$(mktemp)
            trap 'rm -f -- "$pub"' EXIT
            printf '%s\n' "$answer" > "$pub"
            sudo "$cli" server setup --address "$server" --public-key "$pub"
            systemctl is-active niteroi-catalog.socket
            echo 'Catálogo pronto. Continue no cliente. Use folder add e socket add para cadastrar recursos.'
        fi
        exit 0
        ;;
esac
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
    *) echo "Uso: ./install.sh cliente|servidor (ou --client para apenas instalar)" >&2; exit 2 ;;
esac
test -x target/release/ssh-bridge-niteroi
if [ "${1-}" = --install-client-only ]; then
    install -d -m 0755 /etc/systemd/user
    install -m 0644 systemd/niteroi-folder@.service systemd/niteroi-socket@.service /etc/systemd/user/
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
