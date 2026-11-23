# Arquitetura inicial

## Controlador

O binário será executado no Debian servidor. A CLI lê o YAML, valida os dados e apresenta um plano antes de qualquer alteração.

## Papéis

- **Debian servidor:** mantém os sockets e diretórios de origem; as permissões Unix/ACL do usuário SFTP definem o acesso real.
- **Raspberry Pi:** cliente SSH; cria sockets locais encaminhados e monta as pastas remotas.
- **SSHFS:** transporte de arquivos pelo subsistema SFTP do OpenSSH.
- **systemd no Raspberry:** responsável pelo início automático e reinício de túneis/montagens.

## Limite da primeira entrega

A primeira entrega oferece somente `validate` e `plan`. Ela não conecta por SSH, não executa comandos remotos, não instala unidades systemd e não altera permissões.

Antes de implementar `apply`, o contrato de configuração precisa acrescentar:

1. endereço, usuário, chave e `known_hosts` do Raspberry para a conexão de administração;
2. raiz de montagem no Raspberry (o plano anterior indica `~/data`);
3. caminho da chave no Raspberry usada para conectar de volta ao Debian;
4. política de acesso do lado servidor, para que pastas somente leitura também sejam protegidas no sistema de arquivos do Debian.

## Segurança

- Não armazenar conteúdo de chave privada no YAML.
- Não aceitar comandos shell arbitrários em campos do YAML.
- Validar todos os caminhos antes de gerar unidades.
- Não executar `chmod -R` ou `chown -R`.
- `plan` não deve produzir efeitos colaterais.
