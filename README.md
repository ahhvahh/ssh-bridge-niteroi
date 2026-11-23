# SSH Bridge Niterói

CLI Rust para administrar um cadastro de pastas e sockets Unix compartilhados entre máquinas Linux.

## Instalação no Debian

Instale Rust/Cargo conforme https://rustup.rs/ e as ferramentas git, build-essential, openssh-server, acl e util-linux pelo gerenciador de pacotes.

```sh
git clone -b develop https://github.com/ahhvahh/ssh-bridge-niteroi.git
cd ssh-bridge-niteroi
./install.sh
sudoedit /etc/ssh-bridge-niteroi/fornecer-servicos.yaml
```

Execute como usuário normal. O script compila em modo release e solicita sudo somente para a instalação. Para instalar um binário previamente compilado, use sudo ./install.sh --install-only.

O instalador preserva o YAML existente, cria os grupos niteroi-ro, niteroi-rw e niteroi-sockets e instala as unidades systemd.

## Serviço local e inicialização

O instalador registra niteroi-check.service e niteroi-check.timer.
Com iniciar_no_boot: true, o timer é habilitado e iniciado. Com false, fica desabilitado e parado. As verificações periódicas ocorrem a cada minuto e uma execução pontual acontece após 30 segundos da ativação. Cada execução lê o YAML atual; falhas aparecem no journal. A verificação não altera permissões.

Após editar iniciar_no_boot no YAML:

```sh
sudo ssh-bridge-niteroi boot-sync
sudo systemctl start niteroi-check.service
systemctl status niteroi-check.timer
systemctl list-timers niteroi-check.timer
sudo journalctl -u niteroi-check.service -n 50 --no-pager
```

O serviço verifica recursos locais. Túneis e montagens no Raspberry ainda precisam ser implementados.

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

A CLI persiste o cadastro e verifica o acesso no Debian. A configuração remota, montagem SSHFS, encaminhamento de sockets e instalação de unidades systemd no convidado ainda não são executadas pela CLI. Remover um cadastro não revoga permissões Linux nem desmonta recursos existentes. O comando status mostra o cadastro, não conexões ativas.

## Guia de configuração

Veja [Configuração do Debian e do convidado](docs/setup.md).
