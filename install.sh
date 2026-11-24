#!/bin/sh
set -eu
cd "$(dirname "$0")"
# Assistente interativo. Os modos anteriores continuam disponíveis para automação.
cli=/usr/local/bin/ssh-bridge-niteroi
binary=target/release/ssh-bridge-niteroi
if [ ! -f Cargo.toml ] && [ -x ./ssh-bridge-niteroi ]; then
    binary=./ssh-bridge-niteroi
fi
client_build() {
    if [ "$binary" = ./ssh-bridge-niteroi ]; then
        "$binary" --version
    else
        ensure_rust
        cargo build --release
    fi
}
prompt() {
    printf '%s ' "$1" >&2
    IFS= read -r answer || { echo 'Entrada encerrada. Execute o assistente novamente para continuar.' >&2; exit 1; }
}
confirm() {
    prompt "$1 [s/N]"
    case "$answer" in
        s|S|sim|Sim|SIM|y|Y|yes|YES) return 0;;
        *) echo 'Instalação interrompida pelo usuário.' >&2; exit 1;;
    esac
}
ensure_packages() {
    missing=''
    for package in "$@"; do
        if ! dpkg-query -W -f='${db:Status-Status}' "$package" 2>/dev/null | grep -qx installed; then
            missing="$missing $package"
        fi
    done
    if [ -n "$missing" ]; then
        confirm "Atualizar o índice APT e instalar:$missing?"
        command -v sudo >/dev/null || { echo 'É necessário um usuário com sudo.' >&2; exit 1; }
        sudo apt-get update
        # Os nomes são constantes escolhidas pelo instalador.
        # shellcheck disable=SC2086
        sudo apt-get install --no-remove -y $missing
    fi
}
rust_version_ok() {
    command -v cargo >/dev/null && cargo --version >/dev/null 2>&1 &&
        command -v rustc >/dev/null &&
        version=$(rustc --version 2>/dev/null | awk 'NR==1 {print $2}') &&
        [ -n "$version" ] && dpkg --compare-versions "$version" ge 1.85
}
ensure_rust() {
    ensure_packages build-essential ca-certificates
    if [ -x "$HOME/.cargo/bin/cargo" ]; then
        PATH="$HOME/.cargo/bin:$PATH"
        export PATH
    fi
    if rust_version_ok; then return; fi
    if command -v rustup >/dev/null; then
        confirm 'Instalar ou atualizar a toolchain Rust stable?'
        rustup update stable
        rustup default stable
    elif command -v rustc >/dev/null || command -v cargo >/dev/null; then
        confirm 'Atualizar os pacotes Rust/Cargo pelo APT?'
        sudo apt-get update
        sudo apt-get install --no-remove -y rustc cargo
    else
        ensure_packages rustup
        confirm 'Baixar e instalar Rust/Cargo stable para este usuário?'
        rustup toolchain install stable --profile minimal
        rustup default stable
    fi
    if ! rust_version_ok; then
        echo 'Rust/Cargo 1.85 ou superior é necessário. Revise a toolchain ativa.' >&2
        exit 1
    fi
}
ensure_client_packages() {
    ensure_packages openssh-client sshfs fuse3 util-linux coreutils
}
ensure_server_packages() {
    ensure_packages openssh-server openssh-client acl util-linux coreutils
}
case "${1-}" in
    enviar-cliente)
        [ "$#" -ge 3 ] && [ "$#" -le 4 ] || { echo 'Uso: ./install.sh enviar-cliente IP USUARIO [BINARIO]' >&2; exit 2; }
        [ "$(id -u)" -ne 0 ] || { echo 'Execute como usuário normal.' >&2; exit 1; }
        case "$2" in ''|-*|*[!a-zA-Z0-9.-]*) echo 'IP ou hostname inválido.' >&2; exit 2;; esac
        case "$3" in ''|-*|*[!a-zA-Z0-9_-]*) echo 'Usuário inválido.' >&2; exit 2;; esac
        ensure_packages openssh-client coreutils
        dest="$3@$2"
        remote_arch=$(ssh "$dest" 'dpkg --print-architecture')
        remote_machine=$(ssh "$dest" 'uname -m')
        if [ "$#" -eq 4 ]; then
            binary=$4
        elif [ "$(dpkg --print-architecture)" = "$remote_arch" ] && [ "$(uname -m)" = "$remote_machine" ]; then
            client_build
        else
            case "$remote_arch:$remote_machine" in
                arm64:aarch64) target=aarch64-unknown-linux-gnu; linker=aarch64-linux-gnu-gcc; linker_var=CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER;;
                armhf:armv6*) target=arm-unknown-linux-gnueabihf; linker=arm-linux-gnueabihf-gcc; linker_var=CARGO_TARGET_ARM_UNKNOWN_LINUX_GNUEABIHF_LINKER;;
                armhf:arm*|armhf:aarch64) target=armv7-unknown-linux-gnueabihf; linker=arm-linux-gnueabihf-gcc; linker_var=CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER;;
                amd64:x86_64) target=x86_64-unknown-linux-gnu; linker=x86_64-linux-gnu-gcc; linker_var=CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER;;
                *) echo "Arquitetura de destino não suportada: $remote_arch ($remote_machine). Informe um binário compilado para ela." >&2; exit 1;;
            esac
            ensure_rust
            case "$remote_arch" in
                amd64) ensure_packages gcc-x86-64-linux-gnu;;
                *) ensure_packages "gcc-${linker%-gcc}";;
            esac
            if ! command -v rustup >/dev/null; then
                echo "A compilação cruzada requer uma toolchain gerenciada pelo rustup; instale-a ou informe um binário pronto." >&2
                exit 1
            fi
            if ! rustup target list --installed | grep -Fxq "$target"; then
                confirm "Instalar o alvo Rust $target?"
                rustup target add "$target"
            fi
            env "$linker_var=$linker" cargo build --release --target "$target"
            binary="target/$target/release/ssh-bridge-niteroi"
        fi
        test -x "$binary" || { echo 'Binário compilado inexistente ou não executável.' >&2; exit 1; }
        stage=$(mktemp -d)
        trap 'rm -rf -- "$stage"' EXIT
        install -m 0755 "$binary" "$stage/ssh-bridge-niteroi"
        install -m 0755 install.sh "$stage/install.sh"
        install -d "$stage/systemd"
        install -m 0644 systemd/niteroi-folder@.service systemd/niteroi-socket@.service systemd/niteroi-sync.service systemd/niteroi-sync.timer "$stage/systemd/"
        ssh "$dest" 'umask 077; mkdir -p "$HOME/ssh-bridge-niteroi/systemd"'
        scp -p "$stage/ssh-bridge-niteroi" "$stage/install.sh" "$dest:ssh-bridge-niteroi/"
        scp -p "$stage/systemd/"* "$dest:ssh-bridge-niteroi/systemd/"
        ssh "$dest" '"$HOME/ssh-bridge-niteroi/ssh-bridge-niteroi" --version'
        echo "Material enviado. No Raspberry, execute: cd ~/ssh-bridge-niteroi && ./install.sh cliente"
        exit 0
        ;;
    cliente|servidor)
        [ "$#" -eq 1 ] || { echo 'Uso: ./install.sh cliente|servidor' >&2; exit 2; }
        [ "$(id -u)" -ne 0 ] || { echo 'Inicie como usuário normal. O instalador solicitará sudo.' >&2; exit 1; }
        if [ "$1" = cliente ]; then
            ensure_client_packages
            for cmd in ssh ssh-keygen sftp sshfs fusermount3; do
                command -v "$cmd" >/dev/null || { echo "Dependência ausente: $cmd (instale openssh-client e sshfs)." >&2; exit 1; }
            done
            client_build
        else
            [ -f Cargo.toml ] || { echo 'A instalação do servidor requer o projeto completo.' >&2; exit 1; }
            ensure_server_packages
            ensure_rust
            cargo build --release
        fi
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
            systemctl --user daemon-reload
            systemctl --user enable --now niteroi-sync.timer
            echo 'Associe os IDs com client bind ID /caminho/local e execute client mount-all.'
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
        if [ "${1-}" = --client ]; then
            ensure_client_packages
            client_build
            exec sudo -- ./install.sh --install-client-only
        fi
        [ -f Cargo.toml ] || { echo 'Use ./install.sh cliente para instalar o pacote recebido.' >&2; exit 1; }
        ensure_server_packages
        ensure_rust
        cargo build --release
        exec sudo -- ./install.sh --install-only
        ;;
    --install-only|--install-client-only)
        if [ "$(id -u)" -ne 0 ]; then
            echo "A instalação requer sudo." >&2
            exit 1
        fi
        ;;
    *) echo "Uso: ./install.sh cliente|servidor|enviar-cliente IP USUARIO [BINARIO]" >&2; exit 2 ;;
esac
test -x "$binary"
if [ "${1-}" = --install-client-only ]; then
    install -d -m 0755 /etc/systemd/user
    install -m 0644 systemd/niteroi-folder@.service systemd/niteroi-socket@.service systemd/niteroi-sync.service systemd/niteroi-sync.timer /etc/systemd/user/
    install -d -m 0755 /usr/local/bin
    install -m 0755 "$binary" /usr/local/bin/ssh-bridge-niteroi
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
