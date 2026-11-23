# Configuração do Debian e do convidado

## Instalar

Instale git, build-essential, openssh-server, acl, util-linux e Rust/Cargo. Execute:

```sh
git clone -b develop https://github.com/ahhvahh/ssh-bridge-niteroi.git
cd ssh-bridge-niteroi
./install.sh
```

O script compila como usuário normal, instala o binário em /usr/local/bin e registra o timer e serviço no systemd. Um YAML existente é preservado.

## Configurar o servidor

Edite /etc/ssh-bridge-niteroi/fornecer-servicos.yaml com sudoedit. Informe endereço, porta e usuário SSH dedicado que o convidado usará para acessar os recursos, além dos caminhos de chave e known_hosts no cliente. Não inclua conteúdo de chaves privadas. Use iniciar_no_boot: true para monitoramento automático.

Cadastre o convidado com guest add, informando seu endereço e usuário. Cadastre cada pasta com folder add e --permissao ro ou rw e --grupo. Cadastre cada socket com socket add, nome, caminho absoluto no convidado, caminho absoluto no servidor, --grupo e --habilitado. Consulte --help para detalhes. Se já usar um YAML contendo as entradas, edite os grupos nessas entradas em vez de duplicá-las.

O instalador cria três grupos iniciais. Associe ao usuário dedicado somente os grupos necessários e configure o grupo proprietário e as permissões de cada recurso, incluindo o acesso aos diretórios pais. Confira ACLs e permissões dos arquivos existentes e futuros; não execute alterações recursivas indiscriminadamente. Sockets precisam ser recriados pelo serviço de origem com o grupo e modo adequados.

## Conferência

```sh
sudo ssh-bridge-niteroi validate
sudo ssh-bridge-niteroi access check
sudo ssh-bridge-niteroi plan
sudo ssh-bridge-niteroi boot-sync
sudo systemctl start niteroi-check.service
systemctl status niteroi-check.timer
sudo journalctl -u niteroi-check.service -n 50 --no-pager
```

## Convidado

O Raspberry precisa de OpenSSH cliente e SSHFS. Configure uma identidade SSH própria, valide a chave pública do servidor e garanta que a conta dedicada no Debian tenha somente os acessos necessários. O destino de cada pasta deve ser abaixo da raiz de montagem do usuário convidado; defina caminhos locais exclusivos para os sockets. Use montagem somente leitura para as pastas ro.

A etapa de configuração remota automática ainda não existe nesta versão. O timer verifica acessos no Debian, mas não cria montagens SSHFS, túneis ou unidades no Raspberry. Para disponibilização automática será necessário implementar a etapa de aplicação no convidado, com validação de credenciais e permissões e serviços systemd de usuário.
