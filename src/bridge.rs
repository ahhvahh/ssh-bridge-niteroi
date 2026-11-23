use super::{checked_group, checked_path, load_config, Permission};
use clap::Subcommand;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    os::unix::{fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt}, net::UnixStream},
    path::{Component, Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

const ACCOUNT: &str = "niteroi";
const CATALOG: &str = "/run/niteroi-catalog.sock";
const LIMIT: u64 = 1_048_576;

#[derive(Debug, Subcommand)]
pub(super) enum ServerAction {
    /// Configure the dedicated SSH account using the client's public key.
    Setup { #[arg(long)] public_key: PathBuf, #[arg(long)] address: String },
    /// Emit the authorized catalog (used by systemd socket activation).
    Catalog,
}

#[derive(Debug, Subcommand)]
pub(super) enum ClientAction {
    /// Create a local SSH identity and configuration.
    Init { #[arg(long)] server: String, #[arg(long, default_value_t = 22)] port: u16 },
    /// Discover the resources currently available on the server.
    List,
    /// Mount a registered directory using SSHFS.
    Mount { caminho: String },
    /// Mount every folder currently listed in the authorized catalog.
    MountAll,
    /// Open a registered socket for the lifetime of this command.
    Socket { nome: String },
    /// Unmount a local directory previously mounted by the client.
    Unmount { caminho: String },
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct ClientConfig {
    server: String,
    port: u16,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct Catalog {
    version: u8,
    folders: Vec<Folder>,
    sockets: Vec<Socket>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct Folder { path: String, read_only: bool }

#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct Socket { name: String, path: String }

fn error(e: impl std::fmt::Display) -> String { e.to_string() }

fn successful(c: &mut Command) -> Result<(), String> {
    let status = c.status().map_err(error)?;
    if status.success() { Ok(()) } else { Err(format!("Comando falhou: {status}")) }
}

fn safe_host(host: &str) -> bool {
    !host.is_empty() && host.len() <= 253 && !host.starts_with('-')
        && host.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
}

fn safe_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 64 && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn safe_path(p: &str) -> bool {
    p.starts_with('/') && p.len() <= 4096 && p != "/"
        && p.split('/').skip(1).all(|part| !part.is_empty() && part != "." && part != ".."
            && part.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)))
}

fn user_home() -> Result<PathBuf, String> {
    let uid = Command::new("/usr/bin/id").arg("-u").output().map_err(error)?;
    if !uid.status.success() || String::from_utf8_lossy(&uid.stdout).trim() == "0" {
        return Err("Execute os comandos client como usuário normal, sem sudo.".into());
    }
    let home = PathBuf::from(std::env::var_os("HOME").ok_or("HOME ausente")?);
    if !home.is_absolute() || !home.to_str().is_some_and(safe_path) {
        return Err("HOME inválido ou não suportado".into());
    }
    Ok(home)
}

// Reject symlinks in the path below the user's home. Never mount over files.
fn private_dir(base: &Path, relative: &Path) -> Result<PathBuf, String> {
    let mut p = base.to_owned();
    for part in relative.components() {
        let Component::Normal(part) = part else { return Err("Caminho relativo inválido".into()); };
        p.push(part);
        match fs::symlink_metadata(&p) {
            Ok(m) if m.is_dir() && !m.file_type().is_symlink() => {},
            Ok(_) => return Err(format!("Diretório inválido: {}", p.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                use std::os::unix::fs::DirBuilderExt;
                fs::DirBuilder::new().mode(0o700).create(&p).map_err(error)?;
            }
            Err(e) => return Err(error(e)),
        }
    }
    Ok(p)
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut f = fs::OpenOptions::new().create_new(true).write(true).mode(0o600).open(path).map_err(error)?;
    f.write_all(bytes).map_err(error)?;
    f.sync_all().map_err(error)
}

fn client_init(server: String, port: u16) -> Result<(), String> {
    if !safe_host(&server) || port == 0 { return Err("Servidor ou porta inválidos".into()); }
    let home = user_home()?;
    let ssh_dir = private_dir(&home, Path::new(".ssh"))?;
    fs::set_permissions(&ssh_dir, fs::Permissions::from_mode(0o700)).map_err(error)?;
    let key = ssh_dir.join("id_ed25519_niteroi");
    let public = ssh_dir.join("id_ed25519_niteroi.pub");
    if !key.exists() && !public.exists() {
        successful(Command::new("/usr/bin/ssh-keygen").args(["-t", "ed25519", "-N", "", "-C", "niteroi-client", "-f"]).arg(&key))?;
    }
    for p in [&key, &public] {
        if !fs::symlink_metadata(p).map_err(error)?.is_file() { return Err("Identidade SSH inválida".into()); }
    }
    let dir = private_dir(&home, Path::new(".config/ssh-bridge-niteroi"))?;
    let path = dir.join("client.yaml");
    let cfg = ClientConfig { server, port };
    if path.exists() {
        let existing: ClientConfig = yaml_serde::from_str(&fs::read_to_string(&path).map_err(error)?).map_err(error)?;
        if existing.server != cfg.server || existing.port != cfg.port {
            return Err(format!("Configuração existente; revise manualmente {}", path.display()));
        }
    } else {
        write_new(&path, yaml_serde::to_string(&cfg).map_err(error)?.as_bytes())?;
    }
    println!("Chave pública para autorizar no servidor: {}", public.display());
    println!("Valide a impressão digital do host e cadastre-o em ~/.ssh/known_hosts antes de consultar o catálogo.");
    Ok(())
}

struct Client {
    home: PathBuf,
    cfg: ClientConfig,
}

impl Client {
    fn load() -> Result<Self, String> {
        let home = user_home()?;
        let cfg: ClientConfig = yaml_serde::from_str(&fs::read_to_string(home.join(".config/ssh-bridge-niteroi/client.yaml")).map_err(error)?).map_err(error)?;
        if !safe_host(&cfg.server) || cfg.port == 0 { return Err("Servidor ou porta inválidos".into()); }
        Ok(Self { home, cfg })
    }

    fn host(&self) -> String { format!("{ACCOUNT}@{}", self.cfg.server) }

    fn ssh(&self) -> Command {
        let mut c = Command::new("/usr/bin/ssh");
        c.args(["-F", "/dev/null", "-NT", "-p"]).arg(self.cfg.port.to_string())
            .arg("-i").arg(self.home.join(".ssh/id_ed25519_niteroi"))
            .args(["-o", "IdentitiesOnly=yes", "-o", "BatchMode=yes",
                   "-o", "StrictHostKeyChecking=yes", "-o", "ExitOnForwardFailure=yes",
                   "-o", "ConnectTimeout=10", "-o", "ServerAliveInterval=15",
                   "-o", "ServerAliveCountMax=3", "-o", "StreamLocalBindMask=0177"]);
        c
    }

    fn catalog(&self) -> Result<Catalog, String> {
        // A private, randomly named short-lived directory also keeps the Unix path short.
        let out = Command::new("/usr/bin/mktemp").args(["-d", "/tmp/niteroi.XXXXXXXX"]).output().map_err(error)?;
        if !out.status.success() { return Err("Não foi possível criar diretório temporário".into()); }
        let dir = PathBuf::from(String::from_utf8(out.stdout).map_err(error)?.trim());
        let p = dir.join("catalog.sock");
        let result = (|| {
            let forward = format!("{}:{CATALOG}", p.display());
            let mut child = Guard(self.ssh().arg("-L").arg(forward).arg(self.host())
                .stdin(Stdio::null()).stdout(Stdio::null()).spawn().map_err(error)?);
            let start = Instant::now();
            let stream = loop {
                if let Some(s) = child.0.try_wait().map_err(error)? {
                    return Err(format!("Falha na conexão SSH: {s}"));
                }
                match UnixStream::connect(&p) {
                    Ok(s) => break s,
                    Err(_) if start.elapsed() < Duration::from_secs(12) => std::thread::sleep(Duration::from_millis(100)),
                    Err(e) => return Err(format!("Catálogo indisponível: {e}")),
                }
            };
            stream.set_read_timeout(Some(Duration::from_secs(20))).map_err(error)?;
            let mut bytes = Vec::new();
            stream.take(LIMIT + 1).read_to_end(&mut bytes).map_err(error)?;
            if bytes.len() as u64 > LIMIT { return Err("Catálogo grande demais".into()); }
            let catalog: Catalog = yaml_serde::from_slice(&bytes).map_err(error)?;
            if catalog.version != 1 { return Err("Versão do catálogo incompatível".into()); }
            Ok(catalog)
        })();
        let _ = fs::remove_file(&p);
        let _ = fs::remove_dir(&dir);
        result
    }

    fn local_folder(&self, remote: &str) -> Result<PathBuf, String> {
        if !safe_path(remote) { return Err("Caminho remoto inválido".into()); }
        let relative = remote.strip_prefix("/data/").ok_or("O cliente aceita pastas abaixo de /data/")?;
        private_dir(&self.home, &Path::new("data").join(relative))
    }

    fn mount(&self, folder: &Folder) -> Result<(), String> {
        let local = self.local_folder(&folder.path)?;
        if Command::new("/usr/bin/mountpoint").arg("-q").arg(&local).status().map_err(error)?.success() {
            return Err(format!("Já montado: {}. Desmonte antes de alterar a configuração.", local.display()));
        }
        if fs::read_dir(&local).map_err(error)?.next().is_some() { return Err("Ponto de montagem não está vazio".into()); }
        let identity = format!("IdentityFile={}", self.home.join(".ssh/id_ed25519_niteroi").display());
        let mode = if folder.read_only { "ro" } else { "rw" };
        let options = format!("{identity},IdentitiesOnly=yes,BatchMode=yes,StrictHostKeyChecking=yes,ConnectTimeout=10,reconnect,ServerAliveInterval=15,ServerAliveCountMax=3,{mode}");
        successful(Command::new("/usr/bin/sshfs").arg(format!("{}:{}", self.host(), folder.path))
            .arg(&local).arg("-F").arg("/dev/null").arg("-p").arg(self.cfg.port.to_string()).arg("-o").arg(options))?;
        println!("Montado: {} ({mode})", local.display());
        Ok(())
    }

    fn socket(&self, socket: &Socket) -> Result<(), String> {
        if !safe_name(&socket.name) || !safe_path(&socket.path) { return Err("Socket inválido".into()); }
        let dir = private_dir(&self.home, Path::new(".local/share/niteroi/sockets"))?;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).map_err(error)?;
        let local = dir.join(format!("{}.sock", socket.name));
        if local.as_os_str().as_encoded_bytes().len() >= 108 {
            return Err("Caminho de socket muito longo".into());
        }
        if fs::symlink_metadata(&local).is_ok() { return Err(format!("Socket local existente: {}", local.display())); }
        println!("Socket local: {}. Mantenha este comando em execução.", local.display());
        successful(self.ssh().arg("-L").arg(format!("{}:{}", local.display(), socket.path)).arg(self.host()))
    }
}

struct Guard(Child);
impl Drop for Guard {
    fn drop(&mut self) { let _ = self.0.kill(); let _ = self.0.wait(); }
}

pub(super) fn client(action: ClientAction) -> Result<(), String> {
    let action = match action {
        ClientAction::Init { server, port } => return client_init(server, port),
        other => other,
    };
    let c = Client::load()?;
    if let ClientAction::Unmount { caminho } = &action {
        let p = c.local_folder(caminho)?;
        return successful(Command::new("/usr/bin/fusermount3").args(["-u", "--"]).arg(p));
    }
    let catalog = c.catalog()?;
    match action {
        ClientAction::List => {
            for f in catalog.folders { println!("folder {} {}", if f.read_only { "ro" } else { "rw" }, f.path); }
            for s in catalog.sockets { println!("socket {}", s.name); }
            Ok(())
        }
        ClientAction::MountAll => {
            let mut errors = Vec::new();
            for f in &catalog.folders {
                if let Err(e) = c.mount(f) { errors.push(format!("{}: {e}", f.path)); }
            }
            if errors.is_empty() { Ok(()) } else { Err(errors.join("\n")) }
        }
        ClientAction::Mount { caminho } => c.mount(catalog.folders.iter().find(|f| f.path == caminho).ok_or("Pasta não disponível no catálogo")?),
        ClientAction::Socket { nome } => c.socket(catalog.sockets.iter().find(|s| s.name == nome).ok_or("Socket não disponível no catálogo")?),
        _ => unreachable!(),
    }
}

pub(super) fn server(path: &Path, action: ServerAction) -> Result<(), String> {
    if path != Path::new("/etc/ssh-bridge-niteroi/fornecer-servicos.yaml") {
        return Err("Use a configuração instalada do servidor".into());
    }
    match action {
        ServerAction::Setup { public_key, address } => {
            if !safe_host(&address) { return Err("Endereço inválido".into()); }
            let mut cfg = load_config(path)?;
            successful(Command::new("/usr/local/libexec/ssh-bridge-niteroi/setup-server").arg(&public_key))?;
            cfg.servidor_ssh.endereco = address;
            cfg.servidor_ssh.usuario = ACCOUNT.into();
            cfg.servidor_ssh.porta = 22;
            cfg.servidor_ssh.chave_privada = "~/.ssh/id_ed25519_niteroi".into();
            cfg.servidor_ssh.known_hosts = "~/.ssh/known_hosts".into();
            super::save(path, &cfg)?;
            successful(Command::new("/usr/bin/systemctl").args(["enable", "--now", "niteroi-catalog.socket"]))
        }
        ServerAction::Catalog => {
            let c = load_config(path)?;
            if c.versao != 1 || c.servidor_ssh.usuario != ACCOUNT {
                return Err("Servidor precisa ser configurado com a conta dedicada niteroi".into());
            }
            let output = Command::new("/usr/bin/id").args(["-Gn", ACCOUNT]).output().map_err(error)?;
            if !output.status.success() { return Err("Conta dedicada ausente".into()); }
            let groups = String::from_utf8(output.stdout).map_err(error)?;
            let uid = Command::new("/usr/bin/id").args(["-u", ACCOUNT]).output().map_err(error)?;
            if !uid.status.success() || String::from_utf8_lossy(&uid.stdout).trim() == "0" { return Err("Conta dedicada inválida".into()); }
            let allowed = |p: &str, g: &Option<String>, socket: bool, read_only: bool| -> bool {
                if !safe_path(p) || checked_path(p).is_err() || checked_group(g).is_err() { return false; }
                let Some(g) = g else { return false; };
                if g.starts_with('-') { return false; }
                if !groups.split_whitespace().any(|v| v == g) { return false; }
                let Ok(meta) = fs::symlink_metadata(p) else { return false; };
                if socket && !meta.file_type().is_socket() || !socket && !meta.is_dir() { return false; }
                let Ok(group) = Command::new("/usr/bin/getent").args(["group", g.as_str()]).output() else { return false; };
                let gid = String::from_utf8_lossy(&group.stdout).split(':').nth(2).and_then(|v| v.parse::<u32>().ok());
                if !group.status.success() || gid != Some(meta.gid()) { return false; }
                let test = |flag| Command::new("/usr/sbin/runuser").args(["-u", ACCOUNT, "--", "/usr/bin/test", flag, p])
                    .status().map(|s| s.success()).ok();
                let writable = test("-w");
                if socket { writable == Some(true) }
                else { test("-r") == Some(true) && test("-x") == Some(true) && writable == Some(!read_only) }
            };
            let folders = c.pastas_compartilhadas.iter().filter_map(|f| {
                let ro = matches!(&f.permissao, Permission::SomenteLeitura);
                if f.caminho.starts_with("/data/") && allowed(&f.caminho, &f.grupo, false, ro) { Some(Folder { path: f.caminho.clone(), read_only: ro }) } else { None }
            }).collect();
            let sockets = c.sockets.iter().filter(|s| s.habilitado && safe_name(&s.nome) && allowed(&s.remoto, &s.grupo, true, false))
                .map(|s| Socket { name: s.nome.clone(), path: s.remoto.clone() }).collect();
            let data = yaml_serde::to_string(&Catalog { version: 1, folders, sockets }).map_err(error)?;
            if data.len() as u64 > LIMIT { return Err("Catálogo grande demais".into()); }
            std::io::stdout().write_all(data.as_bytes()).map_err(error)
        }
    }
}
