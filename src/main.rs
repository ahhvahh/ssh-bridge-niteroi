mod bridge;
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
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
    #[arg(long = "config", global = true, default_value = "/etc/ssh-bridge-niteroi/fornecer-servicos.yaml")]
    settings: PathBuf,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Lê e valida o arquivo YAML sem alterar o sistema.
    Validate { path: Option<PathBuf> },
    /// Mostra as ações planejadas sem executar comandos SSH ou systemd.
    Plan { path: Option<PathBuf> },
    /// Gerencia sockets cadastrados.
    Socket { #[command(subcommand)] action: SocketAction },
    /// Gerencia pastas cadastradas.
    Folder { #[command(subcommand)] action: FolderAction },
    /// Gerencia convidados cadastrados.
    Guest { #[command(subcommand)] action: GuestAction },
    /// Consulta grupos e permissões locais.
    Access { #[command(subcommand)] action: AccessAction },
    /// Inicializa um arquivo de configuração.
    Init,
    /// Exibe o cadastro atual.
    Status,
    /// Sincroniza o timer local com iniciar_no_boot.
    BootSync,
    /// Configura e publica o catálogo do servidor.
    Server { #[command(subcommand)] action: bridge::ServerAction },
    /// Configura o cliente e acessa os recursos autorizados.
    Client { #[command(subcommand)] action: bridge::ClientAction },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Config {
    versao: u8,
    servidor_ssh: SshServer,
    iniciar_no_boot: bool,
    #[serde(default)]
    sockets: Vec<SocketConfig>,
    #[serde(default)]
    pastas_compartilhadas: Vec<ShareConfig>,
    #[serde(default)]
    convidados: Vec<Guest>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SshServer {
    endereco: String,
    porta: u16,
    usuario: String,
    chave_privada: String,
    known_hosts: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SocketConfig {
    nome: String,
    habilitado: bool,
    local: String,
    remoto: String,
    #[serde(default)]
    grupo: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ShareConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    caminho: String,
    permissao: Permission,
    #[serde(default)]
    grupo: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
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
        Command::Server { action } => bridge::server(&cli.settings, action),
        Command::Client { action } => bridge::client(action),
        Command::BootSync => {
            if cli.settings != Path::new("/etc/ssh-bridge-niteroi/fornecer-servicos.yaml") {
                return Err("boot-sync usa exclusivamente a configuração instalada".into());
            }
            let c = load_config(&cli.settings)?;
            let action = if c.iniciar_no_boot { "enable" } else { "disable" };
            let result = std::process::Command::new("/usr/bin/systemctl")
                .args([action, "--now", "niteroi-check.timer"])
                .status().map_err(|e| e.to_string())?;
            if !result.success() { return Err("falha ao sincronizar timer".into()); }
            println!("Timer no boot: {}", c.iniciar_no_boot);
            Ok(())
        },
        Command::Init => init(&cli.settings),
        Command::Status => { let c = load_config(&cli.settings)?; print_plan(&c); Ok(()) },
        Command::Socket { action } => edit_socket(&cli.settings, action),
        Command::Folder { action } => edit_folder(&cli.settings, action),
        Command::Guest { action } => edit_guest(&cli.settings, action),
        Command::Access { action: AccessAction::Check } => check_access(&load_config(&cli.settings)?),
        Command::Validate { path } => {
            let config = path.unwrap_or(cli.settings);
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
        Command::Plan { path } => {
            let config = path.unwrap_or(cli.settings);
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
    let mut share_ids = HashSet::new();
    for share in &config.pastas_compartilhadas {
        if let Some(id) = &share.id {
            if !bridge::safe_name(id) || !share_ids.insert(id.as_str()) {
                errors.push(format!("ID de pasta inválido ou duplicado: {id}"));
            }
        }
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
        println!("  {} — {} — {}", share.id.as_deref().unwrap_or("sem ID"), share.caminho, share.permissao.label());
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
        "Use server setup no servidor e client init/list/mount/socket no convidado."
    );
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Guest {
    nome: String,
    endereco: String,
    usuario: String,
}

#[derive(Debug, Subcommand)]
enum SocketAction {
    Add { nome: String, local: String, remoto: String, #[arg(long)] grupo: Option<String>, #[arg(long)] habilitado: bool },
    Remove { nome: String },
    List,
}
#[derive(Debug, Subcommand)]
enum FolderAction {
    Add { caminho: String, #[arg(long)] id: String, #[arg(long)] permissao: String, #[arg(long)] grupo: Option<String> },
    Remove { caminho: String },
    List,
}
#[derive(Debug, Subcommand)]
enum GuestAction {
    Add { nome: String, endereco: String, usuario: String },
    Remove { nome: String },
    List,
}
#[derive(Debug, Subcommand)]
enum AccessAction { Check }

fn init(path: &Path) -> Result<(), String> {
    use std::io::Write;
    let mut f = fs::OpenOptions::new().write(true).create_new(true).open(path)
        .map_err(|e| e.to_string())?;
    f.write_all(include_bytes!("../config/fornecer-servicos.example.yaml"))
        .map_err(|e| e.to_string())
}

fn save(path: &Path, config: &Config) -> Result<(), String> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let data = yaml_serde::to_string(config).map_err(|e| e.to_string())?;
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    let mut file = fs::OpenOptions::new().write(true).create_new(true).mode(0o600)
        .open(&tmp).map_err(|e| e.to_string())?;
    let result = file.write_all(data.as_bytes()).and_then(|_| file.sync_all())
        .and_then(|_| fs::rename(&tmp, path));
    if result.is_err() { let _ = fs::remove_file(&tmp); }
    result.map_err(|e| e.to_string())
}

fn checked_path(value: &str) -> Result<(), String> {
    let mut errors = Vec::new();
    validate_absolute_path("caminho", value, &mut errors);
    if value == "/" || value.chars().any(char::is_control) {
        errors.push("caminho inseguro".into());
    }
    if errors.is_empty() { Ok(()) } else { Err(errors.join("\n")) }
}

fn checked_group(grupo: &Option<String>) -> Result<(), String> {
    if let Some(g) = grupo {
        if g.is_empty() || !g.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
            return Err("grupo inválido".into());
        }
    }
    Ok(())
}

fn edit_socket(path: &Path, action: SocketAction) -> Result<(), String> {
    let mut c = load_config(path)?;
    match action {
        SocketAction::List => { for s in &c.sockets { println!("{}: {} -> {} [{}]", s.nome, s.local, s.remoto, s.grupo.as_deref().unwrap_or("sem grupo")); } return Ok(()); }
        SocketAction::Add { nome, local, remoto, grupo, habilitado } => {
            checked_path(&local)?; checked_path(&remoto)?; checked_group(&grupo)?;
            if nome.trim().is_empty() || c.sockets.iter().any(|s| s.nome == nome || s.local == local) { return Err("nome ou socket local duplicado/inválido".into()); }
            c.sockets.push(SocketConfig { nome, local, remoto, grupo, habilitado });
        }
        SocketAction::Remove { nome } => {
            let n = c.sockets.len(); c.sockets.retain(|s| s.nome != nome);
            if n == c.sockets.len() { return Err("socket não encontrado".into()); }
        }
    }
    save(path, &c)
}
fn edit_folder(path: &Path, action: FolderAction) -> Result<(), String> {
    let mut c = load_config(path)?;
    match action {
        FolderAction::List => { for p in &c.pastas_compartilhadas { println!("{}: {} {} [{}]", p.id.as_deref().unwrap_or("sem ID"), p.caminho, p.permissao.label(), p.grupo.as_deref().unwrap_or("sem grupo")); } return Ok(()); }
        FolderAction::Add { caminho, id, permissao, grupo } => {
            checked_path(&caminho)?; checked_group(&grupo)?;
            if !bridge::safe_name(&id) || c.pastas_compartilhadas.iter().any(|p| p.id.as_deref() == Some(id.as_str())) { return Err("ID de pasta inválido ou duplicado".into()); }
            if c.pastas_compartilhadas.iter().any(|p| p.caminho == caminho) { return Err("pasta duplicada".into()); }
            let permissao = match permissao.as_str() { "ro" => Permission::SomenteLeitura, "rw" => Permission::LeituraEscrita, _ => return Err("permissão precisa ser ro ou rw".into()) };
            c.pastas_compartilhadas.push(ShareConfig { id: Some(id), caminho, permissao, grupo });
        }
        FolderAction::Remove { caminho } => {
            let n = c.pastas_compartilhadas.len(); c.pastas_compartilhadas.retain(|p| p.caminho != caminho);
            if n == c.pastas_compartilhadas.len() { return Err("pasta não encontrada".into()); }
        }
    }
    save(path, &c)
}
fn edit_guest(path: &Path, action: GuestAction) -> Result<(), String> {
    let mut c = load_config(path)?;
    match action {
        GuestAction::List => { for g in &c.convidados { println!("{}: {}@{}", g.nome, g.usuario, g.endereco); } return Ok(()); }
        GuestAction::Add { nome, endereco, usuario } => {
            if [&nome, &endereco, &usuario].iter().any(|s| s.trim().is_empty() || s.chars().any(char::is_control)) || c.convidados.iter().any(|g| g.nome == nome) { return Err("convidado inválido ou duplicado".into()); }
            c.convidados.push(Guest { nome, endereco, usuario });
        }
        GuestAction::Remove { nome } => {
            let n = c.convidados.len(); c.convidados.retain(|g| g.nome != nome);
            if n == c.convidados.len() { return Err("convidado não encontrado".into()); }
        }
    }
    save(path, &c)
}

fn check_access(c: &Config) -> Result<(), String> {
    use std::os::unix::fs::{FileTypeExt, MetadataExt};
    use std::process::Command as Process;
    let mut pending = false;
    let uid = Process::new("id").arg("-u").output().map_err(|e| e.to_string())?;
    if String::from_utf8_lossy(&uid.stdout).trim() != "0" { return Err("execute access check com sudo".into()); }
    let memberships = Process::new("id").args(["-Gn", "--", &c.servidor_ssh.usuario])
        .output().map_err(|e| e.to_string())?;
    if !memberships.status.success() { return Err("usuário SSH local não encontrado".into()); }
    let memberships = String::from_utf8_lossy(&memberships.stdout);
    for (path, group, socket) in c.pastas_compartilhadas.iter()
        .map(|p| (p.caminho.as_str(), p.grupo.as_deref(), false))
        .chain(c.sockets.iter().filter(|s| s.habilitado).map(|s| (s.remoto.as_str(), s.grupo.as_deref(), true))) {
        let Some(group) = group else { println!("PENDENTE {path}: grupo não cadastrado"); pending = true; continue; };
        let member = memberships.split_whitespace().any(|g| g == group);
        match fs::symlink_metadata(path) {
            Ok(m) if if socket { m.file_type().is_socket() } else { m.is_dir() } => {
                println!("{path}: grupo esperado={group}, gid={}, modo={:o}, usuário pertence ao grupo={member}", m.gid(), m.mode() & 0o7777);
                let group_entry = Process::new("getent").args(["group", group]).output().map_err(|e| e.to_string())?;
                let group_text = String::from_utf8_lossy(&group_entry.stdout);
                let gid = group_text.split(':').nth(2).and_then(|s| s.parse::<u32>().ok());
                if !member || !group_entry.status.success() || gid != Some(m.gid()) { pending = true; }
                let effective = |flag: &str| -> Result<bool, String> {
                    Process::new("runuser").args(["-u", &c.servidor_ssh.usuario, "--", "/usr/bin/test", flag, path])
                        .status().map(|s| s.success()).map_err(|e| e.to_string())
                };
                let writable = effective("-w")?;
                let readable = effective("-r")?;
                let ok = if socket { writable } else {
                    let rw = c.pastas_compartilhadas.iter().find(|p| p.caminho == path).map(|p| matches!(&p.permissao, Permission::LeituraEscrita)).unwrap_or(false);
                    readable && effective("-x")? && writable == rw
                };
                if !ok { pending = true; println!("PENDENTE: permissões efetivas incompatíveis na entrada"); }
            }
            _ => { println!("PENDENTE {path}: recurso ausente ou tipo incorreto"); pending = true; }
        }
    }
    println!("Diagnóstico da entrada cadastrada; conteúdo recursivo e funcionamento do serviço precisam de avaliação separada.");
    if pending { Err("há pendências de acesso".into()) } else { Ok(()) }
}
