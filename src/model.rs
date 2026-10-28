use crate::database::MachineConnection;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: u64,
    pub connection: MachineConnection,
    pub use_shared_credentials: bool,
    pub remember_password: bool,
    #[serde(skip)]
    pub credential_unavailable: bool,
    #[serde(skip)]
    pub selected: bool,
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct SharedCredentials {
    pub username: String,
    #[serde(skip)]
    pub password: String,
    pub remember_password: bool,
    #[serde(skip)]
    pub credential_unavailable: bool,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub when: u64,
    #[serde(default, skip_serializing_if = "crate::sql::may_contain_credentials")]
    pub sql: String,
    pub targets: usize,
    pub success: usize,
    pub failed: usize,
}
#[derive(Serialize, Deserialize)]
pub struct Workspace {
    pub version: u32,
    pub profiles: Vec<Profile>,
    pub folders: BTreeSet<String>,
    pub shared: SharedCredentials,
    #[serde(default, skip_serializing_if = "crate::sql::may_contain_credentials")]
    pub sql: String,
    pub history: Vec<HistoryEntry>,
    pub read_only: bool,
    pub concurrency: usize,
    pub row_limit: usize,
    pub timeout_seconds: u64,
}
impl Default for Workspace {
    fn default() -> Self {
        Self {
            version: 1,
            profiles: Vec::new(),
            folders: BTreeSet::new(),
            shared: SharedCredentials::default(),
            sql: String::new(),
            history: Vec::new(),
            read_only: true,
            concurrency: 4,
            row_limit: 1000,
            timeout_seconds: 30,
        }
    }
}
impl Workspace {
    pub fn next_id(&self) -> u64 {
        self.profiles
            .iter()
            .map(|profile| profile.id)
            .max()
            .unwrap_or(0)
            + 1
    }
    pub fn resolved(&self, profile: &Profile) -> MachineConnection {
        let mut connection = profile.connection.clone();
        if profile.use_shared_credentials {
            connection.username = self.shared.username.clone();
            connection.password = self.shared.password.clone();
        }
        connection
    }
    pub fn same_target(&self, candidate: &MachineConnection, excluding: Option<u64>) -> bool {
        self.profiles
            .iter()
            .filter(|p| Some(p.id) != excluding)
            .any(|p| {
                p.connection.host.eq_ignore_ascii_case(&candidate.host)
                    && p.connection.port == candidate.port
                    && p.connection.database == candidate.database
                    && self.resolved(p).username == candidate.username
            })
    }
}

pub fn config_path() -> PathBuf {
    if let Some(path) = std::env::var_os("QUERYMASS_CONFIG") {
        return PathBuf::from(path);
    }
    let root = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
    };
    root.unwrap_or_else(std::env::temp_dir)
        .join("querymass")
        .join("workspace.json")
}
pub fn load(path: &Path) -> Result<(Workspace, Vec<String>), String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok((Workspace::default(), Vec::new()));
        }
        Err(error) => {
            return Err(format!(
                "Não foi possível abrir o espaço de trabalho: {error}"
            ));
        }
    };
    let mut workspace: Workspace = serde_json::from_slice(&bytes).map_err(|error| {
        format!("Arquivo de configuração inválido. O original foi preservado: {error}")
    })?;
    if workspace.version != 1 {
        return Err(
            "Versão de configuração não suportada. O arquivo original foi preservado.".into(),
        );
    }
    workspace.concurrency = workspace.concurrency.clamp(1, 16);
    workspace.row_limit = workspace.row_limit.clamp(1, 10000);
    workspace.timeout_seconds = workspace.timeout_seconds.clamp(5, 300);
    let mut warnings = Vec::new();
    for profile in &mut workspace.profiles {
        if profile.remember_password {
            match read_password(&credential_key(path, &format!("profile-{}", profile.id))) {
                Ok(password) => profile.connection.password = password,
                Err(error) => {
                    profile.credential_unavailable = true;
                    warnings.push(format!("{}: {error}", profile.connection.name));
                }
            }
        }
        if !profile.connection.folder.is_empty() {
            workspace.folders.insert(profile.connection.folder.clone());
        }
    }
    if workspace.shared.remember_password {
        match read_password(&credential_key(path, "shared")) {
            Ok(password) => workspace.shared.password = password,
            Err(error) => {
                workspace.shared.credential_unavailable = true;
                warnings.push(error);
            }
        }
    }
    Ok((workspace, warnings))
}
pub fn save(path: &Path, workspace: &Workspace) -> Result<(), String> {
    let parent = path.parent().ok_or("Caminho de configuração inválido.")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temporary = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(workspace).map_err(|error| error.to_string())?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temporary)
        .map_err(|error| error.to_string())?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|error| error.to_string())?;
    fs::rename(&temporary, path).map_err(|error| error.to_string())?;
    Ok(())
}
pub fn write_password(key: &str, password: &str) -> Result<(), String> {
    keyring::Entry::new("io.querymass.desktop", key)
        .and_then(|entry| entry.set_password(password))
        .map_err(|_| {
            "O cofre do sistema está indisponível. A senha permanece apenas nesta sessão.".into()
        })
}
pub fn read_password(key: &str) -> Result<String, String> {
    keyring::Entry::new("io.querymass.desktop", key)
        .and_then(|entry| entry.get_password())
        .map_err(|_| {
            "Não foi possível recuperar a senha do cofre. Informe-a novamente nas conexões.".into()
        })
}
pub fn delete_password(key: &str) -> Result<(), String> {
    match keyring::Entry::new("io.querymass.desktop", key)
        .and_then(|entry| entry.delete_credential())
    {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err("Não foi possível remover a senha do cofre do sistema.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn workspace_never_serializes_credentials() {
        let mut workspace = Workspace::default();
        workspace.shared.password = "sensitive-shared-test".into();
        workspace.profiles.push(Profile {
            id: 1,
            connection: MachineConnection {
                name: "a".into(),
                host: "localhost".into(),
                folder: String::new(),
                port: 3306,
                database: "test".into(),
                username: "user".into(),
                password: "sensitive-machine-test".into(),
                tls: false,
                ca_path: None,
            },
            use_shared_credentials: false,
            remember_password: false,
            credential_unavailable: false,
            selected: true,
        });
        let text = serde_json::to_string(&workspace).unwrap();
        assert!(!text.contains("sensitive-"));
        let restored: Workspace = serde_json::from_str(&text).unwrap();
        assert!(!restored.profiles[0].selected);
        assert!(restored.profiles[0].connection.password.is_empty());
    }
    #[test]
    fn explicit_empty_password_does_not_inherit_shared_password() {
        let mut workspace = Workspace::default();
        workspace.shared.password = "shared-test".into();
        let profile = Profile {
            id: 1,
            connection: MachineConnection {
                name: "x".into(),
                host: "localhost".into(),
                port: 3306,
                folder: String::new(),
                database: String::new(),
                username: "own".into(),
                password: String::new(),
                tls: false,
                ca_path: None,
            },
            use_shared_credentials: false,
            remember_password: false,
            credential_unavailable: false,
            selected: false,
        };
        assert!(workspace.resolved(&profile).password.is_empty());
    }
}

#[cfg(test)]
mod persistence_tests {
    use super::*;
    #[test]
    fn atomic_save_restores_workspace_and_preserves_corrupt_original() {
        let directory =
            std::env::temp_dir().join(format!("querymass-store-{}", std::process::id()));
        let path = directory.join("workspace.json");
        let mut workspace = Workspace {
            sql: "SELECT 123".into(),
            ..Default::default()
        };
        workspace.folders.insert("Empty folder".into());
        save(&path, &workspace).unwrap();
        let (restored, _) = load(&path).unwrap();
        assert_eq!(restored.sql, "SELECT 123");
        assert!(restored.folders.contains("Empty folder"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        fs::write(&path, b"broken-json").unwrap();
        assert!(load(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"broken-json");
        fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn authentication_sql_is_not_serialized() {
        let workspace = Workspace {
            sql: "CREATE USER demo IDENTIFIED BY 'sensitive-literal'".into(),
            ..Default::default()
        };
        assert!(
            !serde_json::to_string(&workspace)
                .unwrap()
                .contains("sensitive-literal")
        );
    }
    #[test]
    #[ignore = "requires an unlocked system credential store"]
    fn system_keyring_roundtrip() {
        let key = format!("querymass-integration-{}", std::process::id());
        write_password(&key, "integration-only-value").unwrap();
        assert_eq!(read_password(&key).unwrap(), "integration-only-value");
        delete_password(&key).unwrap();
        assert!(read_password(&key).is_err());
    }
}

pub fn credential_key(path: &Path, suffix: &str) -> String {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    // FNV provides a stable namespace for workspace paths, not encryption.
    let hash = absolute
        .to_string_lossy()
        .as_bytes()
        .iter()
        .fold(0xcbf29ce484222325u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        });
    format!("workspace-{hash:016x}-{suffix}")
}
pub fn lock_workspace(path: &Path) -> Result<fs::File, String> {
    let parent = path.parent().ok_or("Caminho de configuração inválido.")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path.with_extension("lock"))
        .map_err(|error| error.to_string())?;
    file.try_lock().map_err(|_|"Este espaço de trabalho já está aberto em outra janela. Feche a outra janela antes de abrir novamente.".to_owned())?;
    Ok(file)
}
