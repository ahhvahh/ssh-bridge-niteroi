# SSH Bridge Niterói

CLI em Rust para validar e planejar a configuração de compartilhamento de pastas e sockets Unix entre um servidor Debian e um Raspberry Pi.

## Estado atual

A primeira entrega implementa:

- leitura do YAML com tipos definidos;
- validação de versão, campos SSH, caminhos absolutos e itens duplicados;
- comando `validate` para verificar a configuração;
- comando `plan` para visualizar o que seria configurado, sem alterar as máquinas.

A aplicação **ainda não executa alterações remotas**. O endereço e usuário SSH do Raspberry, o ponto de montagem no convidado e a credencial usada pelo Raspberry para acessar o Debian precisam ser definidos antes de implementar `apply`.

## Compilar

Requisitos: Rust/Cargo e Debian ou outra distribuição Linux.

```sh
cargo build --release
```

O executável fica em `target/release/ssh-bridge-niteroi`.

## Uso

```sh
ssh-bridge-niteroi validate config/fornecer-servicos.yaml
ssh-bridge-niteroi plan config/fornecer-servicos.yaml
```

O exemplo em `config/fornecer-servicos.example.yaml` não contém endereços, usuários ou caminhos do ambiente real. Copie-o para `config/fornecer-servicos.yaml` e preencha os valores. Esse arquivo local já está no `.gitignore`.

## Modelo previsto

O controlador roda no Debian. O Raspberry montará as pastas do Debian via SSHFS/SFTP, com a permissão configurada por pasta, e abrirá os sockets locais por encaminhamento SSH. O início automático será instalado como unidades systemd no Raspberry quando a etapa de aplicação for implementada.

As permissões efetivas também precisam estar corretas no servidor Debian. A opção de montagem somente leitura no Raspberry não substitui as permissões do usuário SSH no servidor.

## Desenvolvimento

- A CLI não usa shell para interpretar comandos recebidos do YAML.
- A aplicação não altera permissões recursivamente.
- Antes de habilitar `apply`, o plano precisa incluir conexão SSH do Raspberry, caminho de montagem e validações das permissões no Debian.
