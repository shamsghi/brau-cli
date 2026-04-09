use std::env;
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Deserializer, Serialize};

const CACHE_FILE_NAME: &str = "catalog-v1.json";
const CATALOG_FORMAT_VERSION: u32 = 2;
const CACHE_MAX_AGE: Duration = Duration::from_secs(12 * 60 * 60);
const REFRESH_LOCK_FILE_NAME: &str = "catalog-refresh.lock";
const REFRESH_STATUS_FILE_NAME: &str = "catalog-refresh-status.json";
const REFRESH_LOCK_MAX_AGE: Duration = Duration::from_secs(30 * 60);
const REFRESH_WAIT_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum HostPlatform {
    Macos,
    Linux,
    Other,
}

impl HostPlatform {
    fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::Macos
        } else if cfg!(any(target_os = "linux", target_os = "android")) {
            Self::Linux
        } else {
            Self::Other
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageKind {
    Formula,
    Cask,
}

impl PackageKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Formula => "formula",
            Self::Cask => "cask",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub kind: PackageKind,
    pub token: String,
    pub full_token: String,
    pub display_names: Vec<String>,
    pub aliases: Vec<String>,
    pub old_names: Vec<String>,
    pub desc: String,
    pub homepage: Option<String>,
    pub version: Option<String>,
    pub tap: Option<String>,
    pub license: Option<String>,
    pub dependencies: Vec<String>,
    pub installed: bool,
    pub outdated: bool,
    pub deprecated: bool,
    pub disabled: bool,
    pub auto_updates: bool,
}

impl Package {
    pub fn install_target(&self) -> &str {
        if self.full_token.is_empty() {
            &self.token
        } else {
            &self.full_token
        }
    }

    pub fn short_status(&self) -> Vec<String> {
        let mut status = Vec::new();

        if let Some(version) = self.version.as_deref() {
            if !version.is_empty() {
                status.push(format!("v{version}"));
            }
        }

        if self.installed {
            status.push("installed".to_string());
        }
        if self.outdated {
            status.push("outdated".to_string());
        }
        if self.deprecated {
            status.push("deprecated".to_string());
        }
        if self.disabled {
            status.push("disabled".to_string());
        }
        if self.auto_updates {
            status.push("auto-updates".to_string());
        }

        status
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Catalog {
    #[serde(default)]
    pub(crate) format_version: u32,
    #[serde(default)]
    pub(crate) host_platform: Option<HostPlatform>,
    pub generated_at: u64,
    #[serde(default)]
    pub brew_state: Option<BrewState>,
    pub items: Vec<Package>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrewState {
    pub taps_root: Option<String>,
    #[serde(default)]
    pub repos: Vec<RepoFingerprint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoFingerprint {
    pub path: String,
    pub head: String,
}

impl Catalog {
    #[cfg(test)]
    pub(crate) fn for_test(items: Vec<Package>) -> Self {
        Self {
            format_version: CATALOG_FORMAT_VERSION,
            host_platform: Some(HostPlatform::current()),
            generated_at: 0,
            brew_state: None,
            items,
        }
    }

    pub fn total_count(&self) -> usize {
        self.items.len()
    }

    pub fn formula_count(&self) -> usize {
        self.items
            .iter()
            .filter(|package| package.kind == PackageKind::Formula)
            .count()
    }

    pub fn cask_count(&self) -> usize {
        self.items
            .iter()
            .filter(|package| package.kind == PackageKind::Cask)
            .count()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LoadOptions {
    pub force_refresh: bool,
    pub allow_stale_fallback: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum CatalogLoadSource {
    Cache,
    Refreshed,
    StaleWhileRefreshing,
    StaleFallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogFreshness {
    Missing,
    Incompatible,
    Fresh,
    UsableStale,
}

#[derive(Debug)]
pub struct CatalogLoad {
    pub catalog: Catalog,
    pub source: CatalogLoadSource,
    pub warning: Option<String>,
}

#[derive(Debug)]
pub struct CacheInspection {
    pub freshness: CatalogFreshness,
    pub catalog: Option<Catalog>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RefreshLock {
    pub pid: u32,
    pub started_at: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RefreshStatus {
    #[serde(default)]
    pub last_started_at: Option<u64>,
    #[serde(default)]
    pub last_completed_at: Option<u64>,
    #[serde(default)]
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum RefreshLockAcquire {
    Acquired(RefreshLock),
    Busy(RefreshLock),
}

impl RefreshStatus {
    pub fn result_for(&self, lock: &RefreshLock) -> Result<(), String> {
        if self.last_started_at != Some(lock.started_at) || self.last_completed_at.is_none() {
            return Err("The catalog refresh ended without recording a result.".to_string());
        }

        match &self.last_error {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }
}

pub fn load_catalog(options: LoadOptions) -> Result<CatalogLoad, String> {
    let cache_path = cache_path()?;
    let inspection = inspect_catalog_cache_at_path(&cache_path)?;

    if !options.force_refresh && inspection.freshness == CatalogFreshness::Fresh {
        return Ok(CatalogLoad {
            catalog: inspection
                .catalog
                .ok_or_else(|| "Fresh cache is missing catalog contents.".to_string())?,
            source: CatalogLoadSource::Cache,
            warning: None,
        });
    }

    match refresh_catalog_at_path(&cache_path) {
        Ok(catalog) => Ok(CatalogLoad {
            catalog,
            source: CatalogLoadSource::Refreshed,
            warning: None,
        }),
        Err(error)
            if options.allow_stale_fallback
                && inspection.freshness == CatalogFreshness::UsableStale =>
        {
            let catalog = inspection
                .catalog
                .ok_or_else(|| format!("{error}\nNo cached package data is available yet."))?;

            Ok(CatalogLoad {
                catalog,
                source: CatalogLoadSource::StaleFallback,
                warning: Some(error),
            })
        }
        Err(error) => Err(error),
    }
}

pub fn inspect_catalog_cache() -> Result<CacheInspection, String> {
    inspect_catalog_cache_at_path(&cache_path()?)
}

pub fn acquire_refresh_lock() -> Result<RefreshLockAcquire, String> {
    let lock_path = refresh_lock_path()?;
    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Failed to create refresh lock directory {}: {error}",
                parent.display()
            )
        })?;
    }

    loop {
        let lock = RefreshLock {
            pid: std::process::id(),
            started_at: now_unix_timestamp(),
        };

        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(file) => {
                serde_json::to_writer(BufWriter::new(file), &lock)
                    .map_err(|error| format!("Failed to serialize refresh lock: {error}"))?;
                return Ok(RefreshLockAcquire::Acquired(lock));
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let existing = match read_refresh_lock()? {
                    Some(existing) => existing,
                    None => continue,
                };

                if refresh_lock_is_reclaimable(&existing, now_unix_timestamp(), pid_is_running) {
                    remove_refresh_lock_if_matches(&existing)?;
                    continue;
                }

                return Ok(RefreshLockAcquire::Busy(existing));
            }
            Err(error) => {
                return Err(format!(
                    "Failed to create refresh lock {}: {error}",
                    lock_path.display()
                ))
            }
        }
    }
}

pub fn handoff_refresh_lock(lock: &RefreshLock, pid: u32) -> Result<RefreshLock, String> {
    let current = read_refresh_lock()?
        .ok_or_else(|| "The refresh lock disappeared before handoff.".to_string())?;
    if current.started_at != lock.started_at {
        return Err("The refresh lock was replaced before handoff.".to_string());
    }

    let updated = RefreshLock {
        pid,
        started_at: lock.started_at,
    };
    write_refresh_lock(&updated)?;
    Ok(updated)
}

pub fn activate_background_refresh(started_at: u64) -> Result<RefreshLock, String> {
    let current = read_refresh_lock()?.ok_or_else(|| {
        "The refresh lock disappeared before the background refresh started.".to_string()
    })?;
    if current.started_at != started_at {
        return Err(
            "The refresh lock was replaced before the background refresh started.".to_string(),
        );
    }

    let lock = if current.pid == std::process::id() {
        current
    } else {
        let updated = RefreshLock {
            pid: std::process::id(),
            started_at,
        };
        write_refresh_lock(&updated)?;
        updated
    };

    mark_refresh_started(&lock)?;
    Ok(lock)
}

pub fn wait_for_refresh(lock: &RefreshLock) -> Result<RefreshStatus, String> {
    loop {
        match read_refresh_lock()? {
            Some(current) if current.started_at == lock.started_at => {
                if refresh_lock_is_reclaimable(&current, now_unix_timestamp(), pid_is_running) {
                    remove_refresh_lock_if_matches(&current)?;
                    break;
                }
                thread::sleep(REFRESH_WAIT_INTERVAL);
            }
            Some(_) | None => break,
        }
    }

    read_refresh_status()
}

pub fn mark_refresh_started(lock: &RefreshLock) -> Result<(), String> {
    let mut status = read_refresh_status()?;
    status.last_started_at = Some(lock.started_at);
    status.last_error = None;
    write_refresh_status(&status)
}

pub fn finish_refresh(lock: &RefreshLock, error: Option<String>) -> Result<(), String> {
    let mut status = read_refresh_status()?;
    status.last_started_at = Some(lock.started_at);
    status.last_completed_at = Some(now_unix_timestamp());
    status.last_error = error;
    write_refresh_status(&status)?;
    remove_refresh_lock_if_matches(lock)
}

pub fn read_refresh_status() -> Result<RefreshStatus, String> {
    let path = refresh_status_path()?;
    let contents = match fs::read(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(RefreshStatus::default())
        }
        Err(error) => {
            return Err(format!(
                "Failed to read refresh status {}: {error}",
                path.display()
            ))
        }
    };

    serde_json::from_slice(&contents)
        .map_err(|error| format!("Failed to parse refresh status {}: {error}", path.display()))
}

pub fn patch_cached_package(package: &Package, installed: bool) -> Result<(), String> {
    let path = cache_path()?;
    let Some(mut catalog) = read_cache_any(&path)? else {
        return Ok(());
    };

    if !patch_catalog_package_state(&mut catalog, package, installed) {
        return Ok(());
    }

    write_catalog_cache(&path, &catalog)
}

fn inspect_catalog_cache_at_path(path: &Path) -> Result<CacheInspection, String> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(CacheInspection {
                freshness: CatalogFreshness::Missing,
                catalog: None,
            })
        }
        Err(error) => {
            return Err(format!(
                "Failed to read cache metadata {}: {error}",
                path.display()
            ))
        }
    };

    let modified = metadata.modified().map_err(|error| {
        format!(
            "Failed to inspect cache timestamp {}: {error}",
            path.display()
        )
    })?;

    let age = SystemTime::now()
        .duration_since(modified)
        .unwrap_or_default();

    let contents = match fs::read(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(CacheInspection {
                freshness: CatalogFreshness::Missing,
                catalog: None,
            })
        }
        Err(error) => return Err(format!("Failed to read cache {}: {error}", path.display())),
    };

    let catalog = match serde_json::from_slice::<Catalog>(&contents) {
        Ok(catalog) => catalog,
        Err(_) => {
            return Ok(CacheInspection {
                freshness: CatalogFreshness::Incompatible,
                catalog: None,
            })
        }
    };

    let freshness = classify_cached_catalog(
        &catalog,
        age,
        catalog
            .brew_state
            .as_ref()
            .map(|state| brew_state_is_current(state).unwrap_or(false)),
    );

    Ok(CacheInspection {
        freshness,
        catalog: matches!(
            freshness,
            CatalogFreshness::Fresh | CatalogFreshness::UsableStale
        )
        .then_some(catalog),
    })
}

fn refresh_catalog_at_path(cache_path: &Path) -> Result<Catalog, String> {
    let output = Command::new("brew")
        .args(["info", "--json=v2", "--eval-all"])
        .output()
        .map_err(|error| format!("Failed to run `brew info --json=v2 --eval-all`: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("Homebrew exited with status {}.", output.status)
        } else {
            format!("Homebrew failed while building the package catalog: {stderr}")
        });
    }

    let response: BrewInfoResponse = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("Failed to parse Homebrew JSON output: {error}"))?;

    let host_platform = HostPlatform::current();
    let mut items = Vec::with_capacity(response.formulae.len() + response.casks.len());
    items.extend(
        response
            .formulae
            .into_iter()
            .filter(|formula| formula.is_compatible_with(host_platform))
            .map(Package::from),
    );
    items.extend(
        response
            .casks
            .into_iter()
            .filter(|cask| cask.is_compatible_with(host_platform))
            .map(Package::from),
    );

    let catalog = Catalog {
        format_version: CATALOG_FORMAT_VERSION,
        host_platform: Some(host_platform),
        generated_at: now_unix_timestamp(),
        brew_state: Some(snapshot_brew_state()?),
        items,
    };

    write_catalog_cache(cache_path, &catalog)?;

    Ok(catalog)
}

fn read_cache_any(path: &Path) -> Result<Option<Catalog>, String> {
    let contents = match fs::read(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Failed to read cache {}: {error}", path.display())),
    };

    let catalog = serde_json::from_slice::<Catalog>(&contents)
        .map_err(|error| format!("Failed to parse cache {}: {error}", path.display()))?;

    Ok(Some(catalog))
}

fn catalog_matches_current_runtime(catalog: &Catalog) -> bool {
    catalog.format_version == CATALOG_FORMAT_VERSION
        && catalog.host_platform == Some(HostPlatform::current())
}

fn classify_cached_catalog(
    catalog: &Catalog,
    age: Duration,
    brew_state_current: Option<bool>,
) -> CatalogFreshness {
    if !catalog_matches_current_runtime(catalog) || catalog.brew_state.is_none() {
        return CatalogFreshness::Incompatible;
    }

    if age <= CACHE_MAX_AGE && matches!(brew_state_current, Some(true)) {
        CatalogFreshness::Fresh
    } else {
        CatalogFreshness::UsableStale
    }
}

fn cache_path() -> Result<PathBuf, String> {
    Ok(cache_dir()?.join(CACHE_FILE_NAME))
}

fn refresh_lock_path() -> Result<PathBuf, String> {
    Ok(cache_dir()?.join(REFRESH_LOCK_FILE_NAME))
}

fn refresh_status_path() -> Result<PathBuf, String> {
    Ok(cache_dir()?.join(REFRESH_STATUS_FILE_NAME))
}

pub fn cache_dir() -> Result<PathBuf, String> {
    let base_dir = if let Ok(path) = env::var("XDG_CACHE_HOME") {
        PathBuf::from(path)
    } else if cfg!(target_os = "macos") {
        home_dir()?.join("Library").join("Caches")
    } else {
        home_dir()?.join(".cache")
    };

    Ok(base_dir.join("brau"))
}

fn home_dir() -> Result<PathBuf, String> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "Could not determine the current home directory.".to_string())
}

fn now_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn write_catalog_cache(path: &Path, catalog: &Catalog) -> Result<(), String> {
    write_json_atomically(path, catalog, "cache file")
}

fn write_refresh_status(status: &RefreshStatus) -> Result<(), String> {
    write_json_atomically(&refresh_status_path()?, status, "refresh status")
}

fn write_refresh_lock(lock: &RefreshLock) -> Result<(), String> {
    write_json_atomically(&refresh_lock_path()?, lock, "refresh lock")
}

fn write_json_atomically<T: Serialize>(path: &Path, value: &T, label: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Failed to create {} directory {}: {error}",
                label,
                parent.display()
            )
        })?;
    }

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_path = path.with_file_name(format!(
        "{}.{}.{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id(),
        unique
    ));
    let file = File::create(&temp_path).map_err(|error| {
        format!(
            "Failed to create {} {}: {error}",
            label,
            temp_path.display()
        )
    })?;
    let writer = BufWriter::new(file);
    serde_json::to_writer(writer, value)
        .map_err(|error| format!("Failed to serialize {} {}: {error}", label, path.display()))?;
    fs::rename(&temp_path, path).map_err(|error| {
        format!(
            "Failed to move {} into place ({} -> {}): {error}",
            label,
            temp_path.display(),
            path.display()
        )
    })
}

fn read_refresh_lock() -> Result<Option<RefreshLock>, String> {
    let path = refresh_lock_path()?;
    let contents = match fs::read(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "Failed to read refresh lock {}: {error}",
                path.display()
            ))
        }
    };

    serde_json::from_slice(&contents)
        .map(Some)
        .map_err(|error| format!("Failed to parse refresh lock {}: {error}", path.display()))
}

fn remove_refresh_lock_if_matches(lock: &RefreshLock) -> Result<(), String> {
    let path = refresh_lock_path()?;
    match read_refresh_lock()? {
        Some(current) if current.started_at == lock.started_at => {
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(format!(
                        "Failed to remove refresh lock {}: {error}",
                        path.display()
                    ))
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn refresh_lock_is_reclaimable(
    lock: &RefreshLock,
    now: u64,
    pid_is_running_fn: impl Fn(u32) -> bool,
) -> bool {
    now.saturating_sub(lock.started_at) > REFRESH_LOCK_MAX_AGE.as_secs()
        || !pid_is_running_fn(lock.pid)
}

fn pid_is_running(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }

    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map(|status| status.success())
        .unwrap_or(true)
}

fn patch_catalog_package_state(catalog: &mut Catalog, package: &Package, installed: bool) -> bool {
    let index = (!package.full_token.is_empty())
        .then(|| {
            catalog
                .items
                .iter()
                .position(|item| item.full_token == package.full_token)
        })
        .flatten()
        .or_else(|| {
            catalog
                .items
                .iter()
                .position(|item| item.token == package.token)
        });

    let Some(index) = index else {
        return false;
    };

    let cached = &mut catalog.items[index];
    cached.installed = installed;
    cached.outdated = false;
    true
}

fn snapshot_brew_state() -> Result<BrewState, String> {
    let root_repo = brew_repository_root()?;
    let taps_root = root_repo.join("Library").join("Taps");

    let mut repos = vec![fingerprint_repo(&root_repo)?];
    for repo in scan_tap_repos(&taps_root)? {
        repos.push(fingerprint_repo(&repo)?);
    }
    repos.sort_by(|left, right| left.path.cmp(&right.path));

    Ok(BrewState {
        taps_root: taps_root
            .exists()
            .then(|| taps_root.to_string_lossy().into_owned()),
        repos,
    })
}

fn brew_state_is_current(state: &BrewState) -> Result<bool, String> {
    if let Some(taps_root) = state.taps_root.as_deref() {
        let taps_root_path = Path::new(taps_root);
        let mut current_tap_paths = scan_tap_repos(taps_root_path)?
            .into_iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        current_tap_paths.sort();

        let mut saved_tap_paths = state
            .repos
            .iter()
            .filter(|repo| repo.path.starts_with(taps_root))
            .map(|repo| repo.path.clone())
            .collect::<Vec<_>>();
        saved_tap_paths.sort();

        if current_tap_paths != saved_tap_paths {
            return Ok(false);
        }
    }

    for repo in &state.repos {
        let path = Path::new(&repo.path);
        if !path.exists() {
            return Ok(false);
        }

        if read_repo_head_signature(path)? != repo.head {
            return Ok(false);
        }
    }

    Ok(true)
}

fn brew_repository_root() -> Result<PathBuf, String> {
    let output = Command::new("brew")
        .arg("--repository")
        .output()
        .map_err(|error| format!("Failed to ask Homebrew for its repository path: {error}"))?;

    if !output.status.success() {
        return Err(format!(
            "Homebrew failed while reporting its repository path with status {}.",
            output.status
        ));
    }

    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        Err("Homebrew returned an empty repository path.".to_string())
    } else {
        Ok(PathBuf::from(path))
    }
}

fn scan_tap_repos(taps_root: &Path) -> Result<Vec<PathBuf>, String> {
    if !taps_root.exists() {
        return Ok(Vec::new());
    }

    let mut repos = Vec::new();
    for owner in fs::read_dir(taps_root).map_err(|error| {
        format!(
            "Failed to read taps directory {}: {error}",
            taps_root.display()
        )
    })? {
        let owner = owner.map_err(|error| {
            format!(
                "Failed to read an entry in taps directory {}: {error}",
                taps_root.display()
            )
        })?;
        let owner_path = owner.path();
        if !owner_path.is_dir() {
            continue;
        }

        for repo in fs::read_dir(&owner_path).map_err(|error| {
            format!(
                "Failed to read tap owner directory {}: {error}",
                owner_path.display()
            )
        })? {
            let repo = repo.map_err(|error| {
                format!(
                    "Failed to read an entry in {}: {error}",
                    owner_path.display()
                )
            })?;
            let repo_path = repo.path();
            if repo_path.is_dir() {
                repos.push(repo_path);
            }
        }
    }

    repos.sort();
    Ok(repos)
}

fn fingerprint_repo(repo_path: &Path) -> Result<RepoFingerprint, String> {
    Ok(RepoFingerprint {
        path: repo_path.to_string_lossy().into_owned(),
        head: read_repo_head_signature(repo_path)?,
    })
}

fn read_repo_head_signature(repo_path: &Path) -> Result<String, String> {
    let git_dir = resolve_git_dir(repo_path)?;
    let head_path = git_dir.join("HEAD");
    let head_contents = fs::read_to_string(&head_path)
        .map_err(|error| format!("Failed to read git HEAD {}: {error}", head_path.display()))?;
    let head = head_contents.trim();

    if let Some(reference) = head.strip_prefix("ref: ").map(str::trim) {
        if let Some(hash) = resolve_ref_hash(&git_dir, reference)? {
            return Ok(format!("{reference}@{hash}"));
        }
        return Ok(reference.to_string());
    }

    Ok(head.to_string())
}

fn resolve_git_dir(repo_path: &Path) -> Result<PathBuf, String> {
    let dot_git = repo_path.join(".git");
    let metadata = fs::metadata(&dot_git)
        .map_err(|error| format!("Failed to inspect {}: {error}", dot_git.display()))?;

    if metadata.is_dir() {
        return Ok(dot_git);
    }

    let contents = fs::read_to_string(&dot_git)
        .map_err(|error| format!("Failed to read {}: {error}", dot_git.display()))?;
    let gitdir = contents
        .trim()
        .strip_prefix("gitdir:")
        .map(str::trim)
        .ok_or_else(|| format!("Unrecognized gitdir format in {}.", dot_git.display()))?;

    let path = PathBuf::from(gitdir);
    Ok(if path.is_absolute() {
        path
    } else {
        repo_path.join(path)
    })
}

fn resolve_ref_hash(git_dir: &Path, reference: &str) -> Result<Option<String>, String> {
    let ref_path = git_dir.join(reference);
    if let Ok(contents) = fs::read_to_string(&ref_path) {
        return Ok(Some(contents.trim().to_string()));
    }

    let packed_refs = git_dir.join("packed-refs");
    let contents = match fs::read_to_string(&packed_refs) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "Failed to read packed refs {}: {error}",
                packed_refs.display()
            ))
        }
    };

    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('^') {
            continue;
        }

        if let Some((hash, ref_name)) = trimmed.split_once(' ') {
            if ref_name == reference {
                return Ok(Some(hash.to_string()));
            }
        }
    }

    Ok(None)
}

#[derive(Debug, Deserialize)]
struct BrewInfoResponse {
    #[serde(default)]
    formulae: Vec<RawFormula>,
    #[serde(default)]
    casks: Vec<RawCask>,
}

#[derive(Debug, Deserialize)]
struct RawFormula {
    name: String,
    #[serde(default)]
    full_name: Option<String>,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    oldnames: Vec<String>,
    #[serde(default)]
    desc: Option<String>,
    #[serde(default)]
    homepage: Option<String>,
    #[serde(default)]
    tap: Option<String>,
    #[serde(default)]
    versions: RawFormulaVersions,
    #[serde(default)]
    license: Option<String>,
    #[serde(default)]
    dependencies: Vec<String>,
    #[serde(default)]
    requirements: Vec<RawRequirement>,
    #[serde(default)]
    installed: Vec<serde_json::Value>,
    #[serde(default)]
    #[serde(deserialize_with = "bool_or_false")]
    outdated: bool,
    #[serde(default)]
    #[serde(deserialize_with = "bool_or_false")]
    deprecated: bool,
    #[serde(default)]
    #[serde(deserialize_with = "bool_or_false")]
    disabled: bool,
}

#[derive(Debug, Default, Deserialize)]
struct RawFormulaVersions {
    #[serde(default)]
    stable: Option<String>,
    #[serde(default)]
    head: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawRequirement {
    name: String,
}

#[derive(Debug, Deserialize)]
struct RawCask {
    token: String,
    #[serde(default)]
    full_token: Option<String>,
    #[serde(default)]
    old_tokens: Vec<String>,
    #[serde(default)]
    name: Vec<String>,
    #[serde(default)]
    desc: Option<String>,
    #[serde(default)]
    homepage: Option<String>,
    #[serde(default)]
    tap: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    depends_on: RawCaskDependsOn,
    #[serde(default)]
    installed: Option<serde_json::Value>,
    #[serde(default)]
    #[serde(deserialize_with = "bool_or_false")]
    outdated: bool,
    #[serde(default)]
    #[serde(deserialize_with = "bool_or_false")]
    deprecated: bool,
    #[serde(default)]
    #[serde(deserialize_with = "bool_or_false")]
    disabled: bool,
    #[serde(default)]
    #[serde(deserialize_with = "bool_or_false")]
    auto_updates: bool,
}

#[derive(Debug, Default, Deserialize)]
struct RawCaskDependsOn {
    #[serde(default)]
    macos: Option<serde_json::Value>,
    #[serde(default)]
    linux: Option<serde_json::Value>,
}

impl RawFormula {
    fn is_compatible_with(&self, platform: HostPlatform) -> bool {
        let requirements = self
            .requirements
            .iter()
            .filter_map(|requirement| requirement_platform(&requirement.name))
            .collect::<Vec<_>>();

        platform_matches(platform, &requirements)
    }
}

impl RawCask {
    fn is_compatible_with(&self, platform: HostPlatform) -> bool {
        self.depends_on.is_compatible_with(platform)
    }
}

impl RawCaskDependsOn {
    fn is_compatible_with(&self, platform: HostPlatform) -> bool {
        let mut requirements = Vec::new();
        if self.macos.is_some() {
            requirements.push(HostPlatform::Macos);
        }
        if self.linux.is_some() {
            requirements.push(HostPlatform::Linux);
        }

        platform_matches(platform, &requirements)
    }
}

fn requirement_platform(name: &str) -> Option<HostPlatform> {
    match name {
        "macos" => Some(HostPlatform::Macos),
        "linux" => Some(HostPlatform::Linux),
        _ => None,
    }
}

fn platform_matches(platform: HostPlatform, requirements: &[HostPlatform]) -> bool {
    requirements.is_empty() || requirements.contains(&platform)
}

impl From<RawFormula> for Package {
    fn from(value: RawFormula) -> Self {
        Self {
            kind: PackageKind::Formula,
            token: value.name,
            full_token: value.full_name.unwrap_or_default(),
            display_names: Vec::new(),
            aliases: value.aliases,
            old_names: value.oldnames,
            desc: value
                .desc
                .unwrap_or_else(|| "No description available.".to_string()),
            homepage: value.homepage,
            version: value
                .versions
                .stable
                .or(value.versions.head)
                .filter(|version| !version.is_empty()),
            tap: value.tap,
            license: value.license,
            dependencies: value.dependencies,
            installed: !value.installed.is_empty(),
            outdated: value.outdated,
            deprecated: value.deprecated,
            disabled: value.disabled,
            auto_updates: false,
        }
    }
}

impl From<RawCask> for Package {
    fn from(value: RawCask) -> Self {
        Self {
            kind: PackageKind::Cask,
            token: value.token,
            full_token: value.full_token.unwrap_or_default(),
            display_names: value.name,
            aliases: Vec::new(),
            old_names: value.old_tokens,
            desc: value
                .desc
                .unwrap_or_else(|| "No description available.".to_string()),
            homepage: value.homepage,
            version: value.version.filter(|version| !version.is_empty()),
            tap: value.tap,
            license: None,
            dependencies: Vec::new(),
            installed: value.installed.is_some(),
            outdated: value.outdated,
            deprecated: value.deprecated,
            disabled: value.disabled,
            auto_updates: value.auto_updates,
        }
    }
}

fn bool_or_false<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<bool>::deserialize(deserializer)?.unwrap_or(false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn raw_formula(name: &str, requirements: &[&str]) -> RawFormula {
        RawFormula {
            name: name.to_string(),
            full_name: None,
            aliases: Vec::new(),
            oldnames: Vec::new(),
            desc: None,
            homepage: None,
            tap: None,
            versions: RawFormulaVersions::default(),
            license: None,
            dependencies: Vec::new(),
            requirements: requirements
                .iter()
                .map(|name| RawRequirement {
                    name: (*name).to_string(),
                })
                .collect(),
            installed: Vec::new(),
            outdated: false,
            deprecated: false,
            disabled: false,
        }
    }

    fn raw_cask(token: &str, macos: bool, linux: bool) -> RawCask {
        RawCask {
            token: token.to_string(),
            full_token: None,
            old_tokens: Vec::new(),
            name: Vec::new(),
            desc: None,
            homepage: None,
            tap: Some("homebrew/cask".to_string()),
            version: None,
            depends_on: RawCaskDependsOn {
                macos: macos.then(|| serde_json::json!({ ">=": ["10.15"] })),
                linux: linux.then_some(serde_json::Value::Bool(true)),
            },
            installed: None,
            outdated: false,
            deprecated: false,
            disabled: false,
            auto_updates: false,
        }
    }

    fn package(token: &str, full_token: &str) -> Package {
        Package {
            kind: PackageKind::Formula,
            token: token.to_string(),
            full_token: full_token.to_string(),
            display_names: Vec::new(),
            aliases: Vec::new(),
            old_names: Vec::new(),
            desc: "test package".to_string(),
            homepage: None,
            version: Some("1.0.0".to_string()),
            tap: None,
            license: None,
            dependencies: Vec::new(),
            installed: false,
            outdated: true,
            deprecated: false,
            disabled: false,
            auto_updates: false,
        }
    }

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(name: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let path = env::temp_dir().join(format!(
                "brau-catalog-tests-{name}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("test temp dir should be created");
            Self { path }
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn write_test_git_repo(path: &Path, hash: &str) {
        fs::create_dir_all(path.join(".git").join("refs").join("heads"))
            .expect("git dir should be created");
        fs::write(path.join(".git").join("HEAD"), "ref: refs/heads/main\n")
            .expect("HEAD should be written");
        fs::write(
            path.join(".git").join("refs").join("heads").join("main"),
            format!("{hash}\n"),
        )
        .expect("ref should be written");
    }

    #[test]
    fn formula_platform_requirements_are_enforced() {
        let macos_only = raw_formula("mas", &["macos"]);
        assert!(macos_only.is_compatible_with(HostPlatform::Macos));
        assert!(!macos_only.is_compatible_with(HostPlatform::Linux));

        let linux_only = raw_formula("glibc", &["linux"]);
        assert!(linux_only.is_compatible_with(HostPlatform::Linux));
        assert!(!linux_only.is_compatible_with(HostPlatform::Macos));

        let cross_platform = raw_formula("ripgrep", &[]);
        assert!(cross_platform.is_compatible_with(HostPlatform::Macos));
        assert!(cross_platform.is_compatible_with(HostPlatform::Linux));
    }

    #[test]
    fn cask_platform_requirements_are_enforced() {
        let macos_only = raw_cask("visual-studio-code", true, false);
        assert!(macos_only.is_compatible_with(HostPlatform::Macos));
        assert!(!macos_only.is_compatible_with(HostPlatform::Linux));

        let linux_only = raw_cask("example-linux-cask", false, true);
        assert!(linux_only.is_compatible_with(HostPlatform::Linux));
        assert!(!linux_only.is_compatible_with(HostPlatform::Macos));

        let unspecified = raw_cask("agnostic-cask", false, false);
        assert!(unspecified.is_compatible_with(HostPlatform::Macos));
        assert!(unspecified.is_compatible_with(HostPlatform::Linux));
    }

    #[test]
    fn fresh_cache_requires_current_catalog_runtime_metadata() {
        let current = Catalog::for_test(Vec::new());
        assert!(catalog_matches_current_runtime(&current));

        let mut outdated = Catalog::for_test(Vec::new());
        outdated.format_version = 0;
        assert!(!catalog_matches_current_runtime(&outdated));
    }

    #[test]
    fn cache_inspection_marks_runtime_mismatch_as_incompatible() {
        let dir = TestDir::new("incompatible");
        let cache_path = dir.path.join("catalog.json");
        let mut catalog = Catalog::for_test(Vec::new());
        catalog.format_version = 0;
        write_catalog_cache(&cache_path, &catalog).expect("cache should be written");

        let inspection = inspect_catalog_cache_at_path(&cache_path).expect("cache inspection");

        assert_eq!(inspection.freshness, CatalogFreshness::Incompatible);
        assert!(inspection.catalog.is_none());
    }

    #[test]
    fn cache_inspection_marks_head_drift_as_usable_stale() {
        let dir = TestDir::new("head-drift");
        let repo_path = dir.path.join("tap");
        write_test_git_repo(&repo_path, "1111111");

        let mut catalog = Catalog::for_test(Vec::new());
        catalog.brew_state = Some(BrewState {
            taps_root: None,
            repos: vec![RepoFingerprint {
                path: repo_path.to_string_lossy().into_owned(),
                head: read_repo_head_signature(&repo_path).expect("initial head"),
            }],
        });

        let cache_path = dir.path.join("catalog.json");
        write_catalog_cache(&cache_path, &catalog).expect("cache should be written");
        fs::write(
            repo_path
                .join(".git")
                .join("refs")
                .join("heads")
                .join("main"),
            "2222222\n",
        )
        .expect("updated head should be written");

        let inspection = inspect_catalog_cache_at_path(&cache_path).expect("cache inspection");

        assert_eq!(inspection.freshness, CatalogFreshness::UsableStale);
        assert!(inspection.catalog.is_some());
    }

    #[test]
    fn refresh_lock_is_reclaimable_when_pid_is_dead() {
        let lock = RefreshLock {
            pid: 4242,
            started_at: now_unix_timestamp(),
        };

        assert!(refresh_lock_is_reclaimable(&lock, lock.started_at, |_| {
            false
        }));
        assert!(!refresh_lock_is_reclaimable(&lock, lock.started_at, |_| {
            true
        }));
    }

    #[test]
    fn refresh_lock_is_reclaimable_when_too_old() {
        let started_at = now_unix_timestamp().saturating_sub(REFRESH_LOCK_MAX_AGE.as_secs() + 1);
        let lock = RefreshLock {
            pid: std::process::id(),
            started_at,
        };

        assert!(refresh_lock_is_reclaimable(
            &lock,
            now_unix_timestamp(),
            |_| true
        ));
    }

    #[test]
    fn patch_catalog_package_state_prefers_full_token() {
        let mut catalog = Catalog::for_test(vec![package("brau", "shamsghi/brau-cli/brau")]);
        let package = package("brau", "shamsghi/brau-cli/brau");

        assert!(patch_catalog_package_state(&mut catalog, &package, true));
        assert!(catalog.items[0].installed);
        assert!(!catalog.items[0].outdated);
    }

    #[test]
    fn patch_catalog_package_state_falls_back_to_plain_token() {
        let mut cached = package("ripgrep", "");
        cached.installed = true;
        let mut catalog = Catalog::for_test(vec![cached]);
        let package = package("ripgrep", "");

        assert!(patch_catalog_package_state(&mut catalog, &package, false));
        assert!(!catalog.items[0].installed);
        assert!(!catalog.items[0].outdated);
    }
}
