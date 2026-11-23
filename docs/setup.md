# Configuração do servidor e do cliente

O servidor Linux anuncia, em um socket Unix local, os recursos cadastrados e autorizados para a conta dedicada niteroi. O cliente consulta esse catálogo via OpenSSH; cada consulta lê o YAML atual e examina grupo e acesso efetivo da conta. O catálogo não recebe comandos, não expõe credenciais e não concede permissões. O SSHFS usa SFTP para pastas; OpenSSH encaminha sockets Unix. Nenhuma porta TCP adicional ao SSH é necessária.

## 1. Instalar no servidor

Instale Rust/Cargo (https://rustup.rs/), git, build-essential, openssh-server, acl e util-linux. No servidor:

```sh
sudo apt install git build-essential openssh-server acl util-linux
sudo systemctl enable --now ssh
git clone -b develop https://github.com/ahhvahh/ssh-bridge-niteroi.git
cd ssh-bridge-niteroi
./install.sh
```

O instalador compila em modo release, preserva a configuração existente, instala o binário em /usr/local/bin, os grupos de recursos e as unidades systemd (incluindo niteroi-catalog.socket).

Antes de confiar em uma conexão, obtenha a impressão digital da chave de host diretamente no console do servidor:

```sh
sudo ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub
```

## 2. Instalar no cliente

Instale Rust/Cargo e as dependências no Raspberry Pi (compile para a arquitetura do próprio sistema):

```sh
sudo apt install git build-essential openssh-client sshfs
git clone -b develop https://github.com/ahhvahh/ssh-bridge-niteroi.git
cd ssh-bridge-niteroi
./install.sh --client
```

Execute como o usuário normal que utilizará os recursos, nunca como root:

```sh
ssh-bridge-niteroi client init --server SERVIDOR
```

Troque SERVIDOR pelo hostname resolvível (ou IP) do Debian. O comando cria ~/.config/ssh-bridge-niteroi/client.yaml e a identidade Ed25519 dedicada ~/.ssh/id_ed25519_niteroi, sem senha para uso não interativo. Proteja a chave privada (não copie para o servidor). Leve somente o arquivo .pub ao administrador do Debian, por um canal autenticado. Para uma porta SSH diferente, use --port e ajuste a política do servidor manualmente.

## 3. Autorizar a chave no servidor

```sh
sudo ssh-bridge-niteroi server setup --address SERVIDOR --public-key /caminho/chave-cliente.pub
```

Esta etapa cria a conta niteroi dedicada, sem shell, e uma política OpenSSH somente por chave. Permite SFTP e encaminhamento local de sockets Unix; desativa encaminhamento TCP, remoto, agente, X11, túnel de rede e terminal. A política é validada com sshd -t e sshd -T antes do reload; um arquivo de configuração existente divergente ou uma chave diferente não são sobrescritos. Uma conta niteroi não criada pelo instalador é recusada para evitar tomar controle de uma conta existente. Nesse caso, o administrador deve revisar e provisionar a conta e a política separadamente.

A chave aceita é única nesta primeira implementação. Não substitua nem reutilize a chave de administração. O comando configura o usuário de acesso no YAML e habilita o catálogo para iniciar no boot. Ele não associa grupos de recursos automaticamente.

## 4. Cadastrar recursos

```sh
sudo ssh-bridge-niteroi folder add /data/project/exemplo --permissao rw --grupo niteroi-rw
sudo ssh-bridge-niteroi folder add /data/project/referencias --permissao ro --grupo niteroi-ro
sudo ssh-bridge-niteroi socket add exemplo /home/usuario/.local/share/niteroi/sockets/exemplo.sock /run/exemplo.sock --grupo niteroi-sockets --habilitado
sudo ssh-bridge-niteroi guest add convidado cliente.local usuario
```

Substitua os exemplos pelos caminhos reais. Se já cadastrou os recursos, edite o YAML existente para preencher os grupos, sem duplicar entradas. O cadastro guest é informativo; a autorização é baseada no usuário SSH e nos grupos.

O administrador deve associar a conta niteroi apenas aos grupos autorizados, garantir o grupo proprietário e as permissões dos recursos e o direito de travessia nos diretórios pais. Use ACLs para os dados existentes e sua herança quando necessário. Para sockets, configure a aplicação proprietária para criá-los com o grupo e modo corretos a cada reinicialização. Não altere permissões recursivamente sem revisar os dados.

O catálogo só inclui pastas abaixo de /data/ e sockets Unix habilitados que passam a conferência de grupo, tipo e acesso efetivo da entrada. Caminhos simples, sem espaços, são suportados nesta versão. Uma pasta ro exige ausência de gravação na entrada; o administrador também precisa verificar os arquivos e subpastas. O catálogo não é uma sandbox de SFTP: outras entradas legíveis pela conta no Linux podem continuar acessíveis diretamente. Para isolamento estrito, use uma conta e ambiente SFTP isolados e restrinja as permissões dos recursos.

```sh
sudo ssh-bridge-niteroi validate
sudo ssh-bridge-niteroi access check
sudo ssh-bridge-niteroi plan
systemctl status niteroi-catalog.socket
```

O campo iniciar_no_boot: true e o comando sudo ssh-bridge-niteroi boot-sync habilitam a verificação periódica. O socket de catálogo é habilitado por server setup independentemente do timer.

## 5. Primeiro acesso no cliente

Como usuário normal, conecte uma vez via SFTP para cadastrar a chave de host:

```sh
sftp -o IdentitiesOnly=yes -i ~/.ssh/id_ed25519_niteroi niteroi@SERVIDOR
```

Compare a impressão digital exibida com aquela obtida diretamente no servidor antes de aceitar. Em seguida, digite quit. O cliente Niterói usa StrictHostKeyChecking=yes e BatchMode=yes, portanto não aceita chaves desconhecidas nem solicita senha.

```sh
ssh-bridge-niteroi client list
ssh-bridge-niteroi client mount /data/project/exemplo
ssh-bridge-niteroi client mount-all
ssh-bridge-niteroi client socket exemplo
```

O ponto de montagem de /data/project/exemplo é ~/data/project/exemplo. Pastas somente leitura são montadas com a opção ro. Mount-all tenta montar todas as pastas anunciadas; pontos já montados são reportados e não substituídos. O socket local usa ~/.local/share/niteroi/sockets/<nome>.sock; a coluna local do cadastro antigo é informativa para este cliente. O comando socket fica em primeiro plano enquanto mantém a conexão; termine com Ctrl+C. Use em outro terminal ou em um serviço systemd do usuário, se necessário. A configuração do catálogo e a chave SSH persistem no boot do servidor, mas a montagem e o túnel no cliente são sob demanda nesta versão.

```sh
ssh-bridge-niteroi client unmount /data/project/exemplo
```

Depois de remover recursos no servidor, novas consultas deixam de anunciá-los. A revogação efetiva exige remover permissões ou a chave SSH, e encerrar conexões já abertas quando necessário. Uma sessão SSH existente pode manter grupos de acesso antigos. A falha na montagem ou no túnel não modifica permissões do servidor.

## 6. Logs e diagnóstico

```sh
sudo journalctl -u niteroi-check.service -n 50 --no-pager
sudo journalctl -u 'niteroi-catalog@*' -n 50 --no-pager
systemctl status ssh niteroi-catalog.socket
```

Se o catálogo vier vazio, confira os grupos, os modos de acesso e o tipo do recurso. Se a conexão SSH falhar, revise hostname, chave de host, chave pública autorizada e a política efetiva de sshd. A chave privada permanece no cliente.
