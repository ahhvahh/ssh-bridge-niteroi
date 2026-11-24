# Configuração do servidor e do cliente

O instalador verifica os pacotes Debian necessários e solicita autorização antes
de atualizar o índice APT ou instalar dependências. Quando é necessário compilar,
verifica Rust/Cargo (mínimo 1.85); se estiverem ausentes, oferece a instalação
da toolchain stable pelo rustup. Para um cliente recebido como binário pronto,
não instala Rust/Cargo. Execute como usuário normal com acesso ao sudo.

## Instalação assistida por papel

Nos dois computadores, atualize a branch develop e execute o script como usuário normal. O assistente usa SSH na porta 22. Instale Rust/Cargo e build-essential nos dois, openssh-server e util-linux no servidor, e openssh-client e sshfs no cliente.

1. No convidado, entre como o usuário que fará as montagens e execute `./install.sh cliente`. Informe o nome resolvível ou IP do servidor. O assistente compila, instala e mostra a chave pública dedicada. Ele aguarda Enter enquanto você configura o servidor.
2. No Debian, execute `./install.sh servidor`, informe o mesmo endereço e cole a linha pública do cliente. Anote a impressão digital de host exibida. A instalação autoriza a identidade, configura o SSH e ativa o socket Unix `/run/niteroi-catalog.sock` agora e nos próximos boots.
3. Cadastre pastas e sockets no Debian conforme a seção de recursos. Conceda à conta niteroi os grupos e permissões dos recursos escolhidos. Confira com `sudo ssh-bridge-niteroi access check`.
4. Retorne ao cliente, pressione Enter e confira a impressão digital antes de aceitar o primeiro acesso. O assistente confirma a autenticação e lista os recursos disponíveis. Use `ssh-bridge-niteroi client list` para consultar novamente, `client mount-all` para montar e `client socket NOME` para abrir um túnel sob demanda.

O catálogo informa os recursos e seus modos de acesso. Os dados passam por SFTP/SSHFS ou pelos túneis Unix. O cliente pode repetir o assistente com a mesma identidade e o mesmo destino.


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

### Enviar o cliente já compilado por SSH

No computador de compilação, dentro do repositório, execute como usuário normal:

```sh
./install.sh enviar-cliente 192.168.1.50 yugo
```

Use o IP real do Raspberry. O instalador consulta a arquitetura do Debian remoto,
compila para ela e envia o executável, o instalador e as unidades systemd de
cliente para `~/ssh-bridge-niteroi/`. Para compilação cruzada, instale previamente
o alvo Rust e o compilador C correspondente. Por exemplo, para Debian ARM64
em um computador Debian AMD64:

```sh
sudo apt install gcc-aarch64-linux-gnu
rustup target add aarch64-unknown-linux-gnu
```

Também é possível informar um executável já compilado:

```sh
./install.sh enviar-cliente 192.168.1.50 yugo /caminho/ssh-bridge-niteroi
```

O envio testa a execução de `--version` no destino. No Raspberry, instale
as dependências openssh-client e sshfs e execute como o usuário de destino:

```sh
cd ~/ssh-bridge-niteroi
./install.sh cliente
```

Nesse modo, o instalador reutiliza o binário recebido e dispensa Cargo no
Raspberry. A instalação permanece assistida e solicita sudo localmente.

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
sudo ssh-bridge-niteroi folder add /data/project/exemplo --id projetos --permissao rw --grupo niteroi-rw
sudo ssh-bridge-niteroi folder add /data/project/referencias --id referencias --permissao ro --grupo niteroi-ro
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
ssh-bridge-niteroi client bind projetos "$HOME/workspace/projetos"
ssh-bridge-niteroi client mount projetos
ssh-bridge-niteroi client mount-all
ssh-bridge-niteroi client socket exemplo
```

O servidor anuncia o ID, o caminho remoto e o modo de acesso no catálogo. O ID é único e aceita letras ASCII, números, hífen e sublinhado (até 64 caracteres). O cliente escolhe um destino absoluto acessível ao seu usuário, inclusive fora de HOME. A pasta deve estar vazia para a montagem e seu caminho não pode conter links simbólicos. A associação persiste em ~/.config/ssh-bridge-niteroi/mounts/<id>. Para alterar o destino, desative a unidade correspondente, desmonte e edite esse arquivo YAML (um caminho absoluto). Mount-all monta os IDs associados e preserva o comportamento anterior para cadastros antigos sem ID. Pastas somente leitura são montadas com ro. O socket local manual usa ~/.local/share/niteroi/sockets/<nome>.sock.

Atualize cliente e servidor antes de cadastrar IDs: o catálogo com IDs utiliza a versão 2. Para migrar uma pasta existente, acrescente o campo id ao cadastro YAML no servidor, associe o destino no cliente e substitua a unidade antiga baseada no caminho pela unidade baseada no ID.

```sh
ssh-bridge-niteroi client unmount projetos
```

Depois de remover recursos no servidor, novas consultas deixam de anunciá-los. A revogação efetiva exige remover permissões ou a chave SSH, e encerrar conexões já abertas quando necessário. Uma sessão SSH existente pode manter grupos de acesso antigos. A falha na montagem ou no túnel não modifica permissões do servidor.

## 6. Logs e diagnóstico

### Reconexão e boot do Raspberry Pi

O assistente de instalação do cliente habilita o timer de sincronização de catálogo.
Para clientes existentes, após atualizar o binário, habilite-o como usuário normal:

```sh
systemctl --user daemon-reload
systemctl --user enable --now niteroi-sync.timer
ssh-bridge-niteroi client refresh
journalctl --user -u niteroi-sync.service -n 50 --no-pager
```

O timer consulta o catálogo a cada minuto após a última execução. O comando
`client refresh` solicita a atualização imediata pelo mesmo serviço. A última
consulta bem-sucedida fica em `~/.config/ssh-bridge-niteroi/catalog-cache.yaml`.
Novos recursos são anunciados no log; pastas novas exigem associação local e
ativação da unidade correspondente. Somente unidades habilitadas de pasta e
socket são reconciliadas: alterações recriam a conexão, recursos removidos
são parados, recursos disponíveis são iniciados se necessário. Em caso de
falha na consulta, as conexões existentes são preservadas. O serviço solicita
as operações ao systemd; consulte o status das unidades para verificar sua conclusão.

Após instalar ou atualizar o cliente e verificar a impressão digital do servidor, execute como yugo. Substitua os exemplos pelos recursos de `client list` e desmonte primeiro uma montagem manual do mesmo caminho.

```sh
systemctl --user daemon-reload
folder_unit=niteroi-folder@projetos.service
systemctl --user enable --now "$folder_unit" niteroi-socket@blender.service
systemctl --user status "$folder_unit" niteroi-socket@blender.service
journalctl --user -u niteroi-socket@blender.service -f
```

Para iniciar mesmo antes do login, um administrador executa `loginctl enable-linger yugo` no Raspberry. Configure o consumidor do socket para usar `$XDG_RUNTIME_DIR/niteroi-blender/blender.sock` (expanda a variável se necessário). O túnel manual mantém seu caminho habitual. Cada unidade consulta o catálogo ao iniciar. Se a rede estiver indisponível ou o processo terminar, o systemd tenta novamente após dez segundos. O SSHFS também usa reconexão interna. Quando o túnel é restabelecido, o cliente MCP precisa abrir uma nova conexão. Para parar e desativar, use `systemctl --user disable --now "$folder_unit" niteroi-socket@blender.service`.

```sh
sudo journalctl -u niteroi-check.service -n 50 --no-pager
sudo journalctl -u 'niteroi-catalog@*' -n 50 --no-pager
systemctl status ssh niteroi-catalog.socket
```

Se o catálogo vier vazio, confira os grupos, os modos de acesso e o tipo do recurso. Se a conexão SSH falhar, revise hostname, chave de host, chave pública autorizada e a política efetiva de sshd. A chave privada permanece no cliente.
