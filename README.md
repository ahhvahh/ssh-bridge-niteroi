# SSH Bridge Niterói

## Instalação assistida por papel

Nos dois computadores, atualize a branch develop e execute o script como usuário normal. O assistente usa SSH na porta 22. Instale Rust/Cargo e build-essential nos dois, openssh-server e util-linux no servidor, e openssh-client e sshfs no cliente.

1. No convidado, entre como o usuário que fará as montagens e execute `./install.sh cliente`. Informe o nome resolvível ou IP do servidor. O assistente compila, instala e mostra a chave pública dedicada. Ele aguarda Enter enquanto você configura o servidor.
2. No Debian, execute `./install.sh servidor`, informe o mesmo endereço e cole a linha pública do cliente. Anote a impressão digital de host exibida. A instalação autoriza a identidade, configura o SSH e ativa o socket Unix `/run/niteroi-catalog.sock` agora e nos próximos boots.
3. Cadastre pastas e sockets no Debian conforme a seção de recursos. Conceda à conta niteroi os grupos e permissões dos recursos escolhidos. Confira com `sudo ssh-bridge-niteroi access check`.
4. Retorne ao cliente, pressione Enter e confira a impressão digital antes de aceitar o primeiro acesso. O assistente confirma a autenticação e lista os recursos disponíveis. Use `ssh-bridge-niteroi client list` para consultar novamente, `client mount-all` para montar e `client socket NOME` para abrir um túnel sob demanda.

O catálogo informa os recursos e seus modos de acesso. Os dados passam por SFTP/SSHFS ou pelos túneis Unix. O cliente pode repetir o assistente com a mesma identidade e o mesmo destino.


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

O serviço verifica recursos locais. O catálogo é disponibilizado por um socket Unix gerenciado pelo systemd após o comando server setup. A propriedade iniciar_no_boot controla o timer de diagnóstico, não a ativação do catálogo.

## Cadastro

```sh
sudo ssh-bridge-niteroi guest add raspberry raspberry.local yugo
sudo ssh-bridge-niteroi folder add /data/project/projetos --permissao rw --grupo niteroi-rw
sudo ssh-bridge-niteroi folder add /data/project/referencias --permissao ro --grupo niteroi-ro
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

A CLI administra o cadastro no Debian e oferece um cliente para consultar o catálogo e montar pastas ou abrir sockets sob demanda. A autenticação usa uma conta SSH dedicada sem shell interativo. Remover um cadastro não revoga permissões Linux nem desmonta recursos existentes. Para iniciar recursos no boot e recuperar túneis após uma queda, habilite as unidades de usuário descritas no [guia de reconexão](docs/setup.md#reconexão-e-boot-do-raspberry-pi). O comando status mostra o cadastro, não conexões ativas.

## Guia de configuração

Veja [Configuração do Debian e do convidado](docs/setup.md).

## Cliente e servidor

O guia [Configuração do Debian e do convidado](docs/setup.md) explica a instalação e o cadastro da chave SSH. No cliente, use `client init`, `client list`, `client mount`, `client mount-all`, `client unmount` e `client socket`. Em outro Linux, compile usando `./install.sh --client`. O cliente roda sem sudo após a instalação.
