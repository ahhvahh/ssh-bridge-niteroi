# Arquitetura

## Servidor e cliente

O mesmo binário Rust funciona no Debian servidor e em uma máquina convidada Linux. O servidor mantém o YAML em /etc/ssh-bridge-niteroi/fornecer-servicos.yaml e usa systemd para verificação periódica e ativação do catálogo por socket Unix.

1. No cliente, a identidade Ed25519 é gerada para a conta de uso local.
2. O administrador do servidor cadastra a chave pública usando server setup. A conta dedicada niteroi utiliza autenticação por chave, SFTP e encaminhamento local de sockets Unix, sem terminal interativo.
3. client list conecta por SSH e consulta /run/niteroi-catalog.sock. O systemd inicia uma instância curta que lê o cadastro atualizado, verifica a conta e apresenta os recursos que passam nos controles locais.
4. client mount e client mount-all montam somente pastas anunciadas via SSHFS, abaixo de ~/data. client socket abre o encaminhamento de um socket anunciado, abaixo de ~/.local/share/niteroi/sockets.

## Limites

- O socket de catálogo não recebe comandos nem dados de configuração do cliente. A chave privada fica no cliente.
- A autorização efetiva é exercida pelas permissões Unix/ACLs e pelo OpenSSH no servidor. O catálogo filtra o cadastro; não isola o sistema de arquivos SFTP e não substitui ACLs nem políticas do serviço de origem.
- O cliente monta recursos sob demanda. A inicialização automática das montagens e túneis no cliente não é instalada nesta versão.
- Remover uma entrada controla futuras consultas, não revoga permissões e sessões existentes.
- Nesta versão há uma identidade de convidado cadastrada por servidor. Múltiplas identidades e políticas por convidado exigirão um modelo de autorização separado.

## Segurança operacional

- A autenticação do host SSH é verificada contra known_hosts; não desative a conferência.
- Os comandos client são executados como usuário sem privilégios.
- A administração das contas, dos grupos e do cadastro exige privilégios locais no servidor.
- Não armazenar conteúdo de chave privada no YAML.
- Não aceitar comandos shell arbitrários em campos do YAML.
- Não executar chmod -R ou chown -R indiscriminadamente.
- plan não produz efeitos colaterais.
