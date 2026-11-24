#!/bin/sh
# Administrative enrollment of a single, dedicated SSH identity.
set -eu
umask 077
export PATH=/usr/sbin:/usr/bin:/sbin:/bin
[ "$(id -u)" -eq 0 ] || { echo 'Execute server setup com sudo.' >&2; exit 1; }
[ "$#" -eq 1 ] || { echo 'Informe a chave pública Ed25519 do convidado.' >&2; exit 1; }
pub="$1"
[ -f "$pub" ] || { echo 'Chave pública não encontrada.' >&2; exit 1; }
[ "$(wc -c < "$pub")" -le 4096 ] || { echo 'Chave muito grande.' >&2; exit 1; }
key=$(awk 'NF { if (++n != 1 || $1 != "ssh-ed25519" || $2 !~ /^[A-Za-z0-9+\/=]+$/) exit 1; print $1 " " $2 } END { if (n != 1) exit 1 }' "$pub") || { echo 'Chave pública inválida.' >&2; exit 1; }
work=$(mktemp -d)
drop=/etc/ssh/sshd_config.d/00-niteroi.conf
auth=/etc/ssh/niteroi_authorized_keys
marker=/etc/ssh-bridge-niteroi/managed-account
new_drop=0
replaced_drop=0
new_auth=0
committed=0
cleanup() {
    if [ "$committed" -eq 0 ]; then
        if [ "$new_drop" -eq 1 ]; then rm -f -- "$drop"; fi
        if [ "$replaced_drop" -eq 1 ]; then install -o root -g root -m 0644 "$work/previous.conf" "$drop"; fi
        if [ "$new_auth" -eq 1 ]; then rm -f -- "$auth"; fi
    fi
    rm -f -- "$work/key.pub" "$work/authorized_keys" "$work/sshd.conf" "$work/previous.conf" "$work/normalized-old" "$work/normalized-new"
    rmdir -- "$work"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM
printf '%s\n' "$key" > "$work/key.pub"
ssh-keygen -l -f "$work/key.pub" >/dev/null
printf '%s %s\n' 'restrict,port-forwarding' "$key" > "$work/authorized_keys"
cat > "$work/sshd.conf" <<'SSHD'
Match User niteroi
    AuthorizedKeysFile /etc/ssh/niteroi_authorized_keys
    AuthenticationMethods publickey
    PubkeyAuthentication yes
    PasswordAuthentication no
    KbdInteractiveAuthentication no
    ForceCommand internal-sftp
    AllowTcpForwarding local
    AllowStreamLocalForwarding local
    AllowAgentForwarding no
    X11Forwarding no
    PermitTTY no
    PermitTunnel no
    PermitUserRC no
Match all
SSHD
sshd -t
systemctl is-active --quiet ssh.service || { echo 'O serviço ssh.service deve estar ativo.' >&2; exit 1; }
[ -d /etc/ssh/sshd_config.d ] || { echo 'Diretório de configuração do OpenSSH ausente.' >&2; exit 1; }
getent group niteroi-catalog >/dev/null || { echo 'Execute o instalador antes.' >&2; exit 1; }
if getent passwd niteroi >/dev/null; then
    [ -f "$marker" ] || { echo 'Conta niteroi existente e não gerenciada. Revise-a manualmente antes de adotá-la.' >&2; exit 1; }
    [ "$(id -u niteroi)" -ne 0 ] || exit 1
    [ "$(getent passwd niteroi | cut -d: -f7)" = /usr/sbin/nologin ] || { echo 'Shell inesperado da conta.' >&2; exit 1; }
else
    useradd --system --user-group --create-home --home-dir /var/lib/niteroi --shell /usr/sbin/nologin niteroi
    install -m 0600 /dev/null "$marker"
fi
# Never overwrite a different enrolled identity or administrator configuration.
if [ -e "$auth" ]; then
    cmp -s "$work/authorized_keys" "$auth" || { echo 'Chave já cadastrada é diferente. Rotação requer revisão local.' >&2; exit 1; }
else
    install -o root -g root -m 0644 "$work/authorized_keys" "$auth"
    new_auth=1
fi
# Only migrate the original managed policy and the known diagnostic variant.
normalize_policy() {
    awk '{
        if ($1 == "LogLevel" && $2 == "DEBUG1" && NF == 2) next;
        if ($1 == "AllowTcpForwarding" && ($2 == "no" || $2 == "local") && NF == 2)
            $2 = "local";
        if (NF) { $1=$1; if (!seen[$0]++) print }
    }' "$1"
}
if [ -e "$drop" ]; then
    normalize_policy "$drop" > "$work/normalized-old"
    normalize_policy "$work/sshd.conf" > "$work/normalized-new"
    cmp -s "$work/normalized-old" "$work/normalized-new" || {
        echo 'Configuração SSH existente difere da política gerenciada. Revise-a antes de continuar.' >&2
        exit 1
    }
fi

# Start the account-specific guard before enabling OpenSSH local forwarding.
systemctl daemon-reload
systemctl reload-or-restart niteroi-guard.service
systemctl is-active --quiet niteroi-guard.service
/usr/sbin/nft list table inet niteroi_guard >/dev/null
install -d -m 0755 /etc/systemd/system/ssh.service.d
dependency=/etc/systemd/system/ssh.service.d/90-niteroi-guard.conf
if [ ! -e "$dependency" ]; then
    printf '%s\n' '[Unit]' 'Requires=niteroi-guard.service' 'After=niteroi-guard.service' '[Service]' 'ExecStartPre=/usr/local/libexec/ssh-bridge-niteroi/niteroi-guard' > "$dependency"
else
    printf '%s\n' '[Unit]' 'Requires=niteroi-guard.service' 'After=niteroi-guard.service' '[Service]' 'ExecStartPre=/usr/local/libexec/ssh-bridge-niteroi/niteroi-guard' | cmp -s - "$dependency" || {
        echo 'Dependência systemd existente divergente.' >&2; exit 1;
    }
fi
systemctl daemon-reload
if [ -e "$drop" ]; then
    if ! cmp -s "$work/sshd.conf" "$drop"; then
        cp -p "$drop" "$work/previous.conf"
        replaced_drop=1
        install -o root -g root -m 0644 "$work/sshd.conf" "$drop"
    fi
else
    install -o root -g root -m 0644 "$work/sshd.conf" "$drop"
    new_drop=1
fi
sshd -t
effective=$(sshd -T -C user=niteroi,host=localhost,addr=127.0.0.1)
for expected in 'authenticationmethods publickey' 'passwordauthentication no' 'kbdinteractiveauthentication no' 'forcecommand internal-sftp' 'allowtcpforwarding local' 'allowstreamlocalforwarding local' 'authorizedkeysfile /etc/ssh/niteroi_authorized_keys' 'permittty no'; do
    printf '%s\n' "$effective" | grep -Fxq "$expected" || { echo "Configuração efetiva incompatível: $expected" >&2; exit 1; }
done
usermod -aG niteroi-catalog niteroi
systemctl reload ssh.service
committed=1
echo 'Conta SSH dedicada configurada. Conceda somente os grupos dos recursos autorizados.'
