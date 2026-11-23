# SSH Bridge Niterói

CLI Rust para administrar um cadastro de pastas e sockets Unix compartilhados entre máquinas Linux.

## Instalação no Debian

Instale Rust/Cargo conforme https://rustup.rs/ e as ferramentas git, build-essential, openssh-server, acl e util-linux pelo gerenciador de pacotes.

```sh
git clone -b develop https://github.com/ahhvahh/ssh-bridge-niteroi.git
cd ssh-bridge-niteroi
cargo build --release
sudo ./install.sh
sudoedit /etc/ssh-bridge-niteroi/fornecer-servicos.yaml
```

O instalador preserva o YAML existente e cria os grupos niteroi-ro, niteroi-rw e niteroi-sockets.

## Cadastro

```sh
sudo ssh-bridge-niteroi guest add raspberry raspberry.local yugo
sudo ssh-bridge-niteroi folder add /srv/projetos --permissao rw --grupo niteroi-rw
sudo ssh-bridge-niteroi folder add /srv/referencias --permissao ro --grupo niteroi-ro
sudo ssh-bridge-niteroi socket add exemplo /run/user/1000/exemplo.sock /run/exemplo.sock --grupo niteroi-sockets --habilitado
sudo ssh-bridge-niteroi folder list
sudo ssh-bridge-niteroi socket list
sudo ssh-bridge-niteroi guest list
sudo ssh-bridge-niteroi validate
sudo ssh-bridge-niteroi access check
sudo ssh-bridge-niteroi plan
```

Remova pelo nome do socket/convidado ou caminho da pasta, usando os subcomandos respectivos remove.
O parâmetro global --config /caminho/arquivo.yaml permite usar outro arquivo. O comando init cria um novo YAML sem sobrescrever um existente.

## Permissões

Cadastre grupos específicos por recurso quando precisar de isolamento. O administrador deve configurar as permissões no servidor e o usuário SSH com privilégios mínimos; a montagem ro no cliente não substitui as permissões no servidor. O comando access check verifica grupo proprietário, associação ao grupo, tipo e acesso efetivo à entrada cadastrada. Não certifica o conteúdo recursivo.

## Estado atual

A CLI persiste o cadastro e verifica o acesso no Debian. A configuração remota, montagem SSHFS, encaminhamento de sockets e instalação de unidades systemd ainda não são executadas pela CLI. Remover um cadastro não revoga permissões Linux nem desmonta recursos existentes. O comando status mostra o cadastro, não conexões ativas.
