use clap::{Parser, Subcommand};
use serde::Deserialize;
use std::{
    collections::HashSet,
    fs,
    path::{Component, Path, PathBuf},
    process::ExitCode,
};

#[derive(Debug, Parser)]
#[command(name = "ssh-bridge-niteroi", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Lê e valida o arquivo YAML sem alterar o sistema.
    Validate { config: PathBuf },
    /// Mostra as ações planejadas sem executar comandos SSH ou systemd.
    Plan { config: PathBuf },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    versao: u8,
    servidor_ssh: SshServer,
    iniciar_no_boot: bool,
    #[serde(default)]
    sockets: Vec<SocketConfig>,
    #[serde(default)]
    pastas_compartilhadas: Vec<ShareConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SshServer {
    endereco: String,
    porta: u16,
    usuario: String,
    chave_privada: String,
    known_hosts: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SocketConfig {
    nome: String,
    habilitado: bool,
    local: String,
    remoto: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ShareConfig {
    caminho: String,
    permissao: Permission,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Permission {
    LeituraEscrita,
    SomenteLeitura,
}

impl Permission {
    fn label(&self) -> &'static str {
        match self {
            Self::LeituraEscrita => "leitura e gravação",
            Self::SomenteLeitura => "somente leitura",
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("erro: {message}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();
    match cli.command {
        Command::Validate { config } => {
            let parsed = load_config(&config)?;
            let errors = validate(&parsed);
            if errors.is_empty() {
                println!("YAML válido: {}", config.display());
                println!(
                    "{} pasta(s), {} socket(s); iniciar no boot: {}",
                    parsed.pastas_compartilhadas.len(),
                    parsed.sockets.len(),
                    parsed.iniciar_no_boot
                );
                Ok(())
            } else {
                Err(errors.join("\n"))
            }
        }
        Command::Plan { config } => {
            let config = load_config(&config)?;
            let errors = validate(&config);
            if !errors.is_empty() {
                return Err(errors.join("\n"));
            }
            print_plan(&config);
            Ok(())
        }
    }
}

fn load_config(path: &Path) -> Result<Config, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("não foi possível ler {}: {error}", path.display()))?;
    yaml_serde::from_str(&text)
        .map_err(|error| format!("YAML inválido em {}: {error}", path.display()))
}

fn validate(config: &Config) -> Vec<String> {
    let mut errors = Vec::new();

    if config.versao != 1 {
        errors.push(format!("versao {} não suportada; esperado: 1", config.versao));
    }

    for (label, value) in [
        ("servidor_ssh.endereco", config.servidor_ssh.endereco.as_str()),
        ("servidor_ssh.usuario", config.servidor_ssh.usuario.as_str()),
        ("servidor_ssh.chave_privada", config.servidor_ssh.chave_privada.as_str()),
        ("servidor_ssh.known_hosts", config.servidor_ssh.known_hosts.as_str()),
    ] {
        if value.trim().is_empty() || looks_unfilled(value) {
            errors.push(format!("{label} está vazio ou ainda contém um valor de exemplo"));
        }
    }

    if config.servidor_ssh.porta == 0 {
        errors.push("servidor_ssh.porta precisa estar entre 1 e 65535".to_string());
    }

    let mut socket_names = HashSet::new();
    for socket in &config.sockets {
        if socket.nome.trim().is_empty() {
            errors.push("socket com nome vazio".to_string());
        } else if !socket_names.insert(socket.nome.as_str()) {
            errors.push(format!("nome de socket duplicado: {}", socket.nome));
        }

        if socket.habilitado {
            validate_absolute_path("socket.local", &socket.local, &mut errors);
            validate_absolute_path("socket.remoto", &socket.remoto, &mut errors);
            if looks_unfilled(&socket.local) || looks_unfilled(&socket.remoto) {
                errors.push(format!(
                    "socket {} está habilitado, mas ainda contém caminhos de exemplo",
                    socket.nome
                ));
            }
        }
    }

    let mut share_paths = HashSet::new();
    for share in &config.pastas_compartilhadas {
        validate_absolute_path("pastas_compartilhadas.caminho", &share.caminho, &mut errors);
        if !share_paths.insert(share.caminho.as_str()) {
            errors.push(format!("pasta compartilhada duplicada: {}", share.caminho));
        }
    }

    errors
}

fn validate_absolute_path(label: &str, value: &str, errors: &mut Vec<String>) {
    let path = Path::new(value);
    if !path.is_absolute() {
        errors.push(format!("{label} precisa ser um caminho absoluto: {value}"));
        return;
    }
    if path.components().any(|component| component == Component::ParentDir) {
        errors.push(format!("{label} não pode conter '..': {value}"));
    }
}

fn looks_unfilled(value: &str) -> bool {
    let upper = value.to_ascii_uppercase();
    upper.contains("PREENCHER")
        || upper.contains("SERVIDOR_OU_IP")
        || upper.contains("USUARIO_SSH")
        || upper.contains("/CAMINHO/PARA/")
}

fn print_plan(config: &Config) {
    println!("Plano de configuração — nenhuma alteração será aplicada.");
    println!(
        "Servidor SSH: {}:{} (usuário {})",
        config.servidor_ssh.endereco, config.servidor_ssh.porta, config.servidor_ssh.usuario
    );
    println!(
        "Início automático planejado no Raspberry: {}",
        config.iniciar_no_boot
    );

    println!("Pastas:");
    for share in &config.pastas_compartilhadas {
        println!("  {} — {}", share.caminho, share.permissao.label());
    }

    println!("Sockets habilitados:");
    let mut enabled_count = 0;
    for socket in config.sockets.iter().filter(|socket| socket.habilitado) {
        enabled_count += 1;
        println!("  {} — {} -> {}", socket.nome, socket.local, socket.remoto);
    }
    if enabled_count == 0 {
        println!("  nenhum");
    }

    println!(
        "Etapa pendente: conexão SSH com o Raspberry, pontos de montagem e aplicação systemd."
    );
}
