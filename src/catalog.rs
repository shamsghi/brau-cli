use std::env;
use std::fs;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Deserializer, Serialize};

const CACHE_FILE_NAME: &str = "catalog-v1.json";
const CACHE_MAX_AGE: Duration = Duration::from_secs(12 * 60 * 60);

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
    StaleFallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheStatus {
    Missing,
    Fresh,
    Stale,
}

#[derive(Debug)]
pub struct CatalogLoad {
    pub catalog: Catalog,
    pub source: CatalogLoadSource,
    pub warning: Option<String>,
}

pub fn load_catalog(options: LoadOptions) -> Result<CatalogLoad, String> {
    let cache_path = cache_path()?;

    if !options.force_refresh {
        if let Some(catalog) = read_cache_if_fresh(&cache_path)? {
            return Ok(CatalogLoad {
                catalog,
                source: CatalogLoadSource::Cache,
                warning: None,
            });
        }
    }

    match refresh_catalog(&cache_path) {
        Ok(catalog) => Ok(CatalogLoad {
            catalog,
            source: CatalogLoadSource::Refreshed,
            warning: None,
        }),
        Err(error) if options.allow_stale_fallback => {
            let catalog = read_cache_any(&cache_path)?
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

pub fn cache_status() -> Result<CacheStatus, String> {
    let path = cache_path()?;
    let metadata = match fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(CacheStatus::Missing)
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

    Ok(if age <= CACHE_MAX_AGE {
        CacheStatus::Fresh
    } else {
        CacheStatus::Stale
    })
}

fn refresh_catalog(cache_path: &Path) -> Result<Catalog, String> {
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

    let mut items = Vec::with_capacity(response.formulae.len() + response.casks.len());
    items.extend(response.formulae.into_iter().map(Package::from));
    items.extend(response.casks.into_iter().map(Package::from));

    let catalog = Catalog {
        generated_at: now_unix_timestamp(),
        brew_state: snapshot_brew_state().ok(),
        items,
    };

    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Failed to create cache directory {}: {error}",
                parent.display()
            )
        })?;
    }

    let temp_path = cache_path.with_extension("tmp");
    let file = File::create(&temp_path).map_err(|error| {
        format!(
            "Failed to create cache file {}: {error}",
            temp_path.display()
        )
    })?;
    let writer = BufWriter::new(file);
    serde_json::to_writer(writer, &catalog)
        .map_err(|error| format!("Failed to serialize the package catalog: {error}"))?;
    fs::rename(&temp_path, cache_path).map_err(|error| {
        format!(
            "Failed to move cache file into place ({} -> {}): {error}",
            temp_path.display(),
            cache_path.display()
        )
    })?;

    Ok(catalog)
}

fn read_cache_if_fresh(path: &Path) -> Result<Option<Catalog>, String> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
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

    if age > CACHE_MAX_AGE {
        return Ok(None);
    }

    let Some(catalog) = read_cache_any(path)? else {
        return Ok(None);
    };

    match catalog.brew_state.as_ref() {
        Some(state) => match brew_state_is_current(state) {
            Ok(true) => Ok(Some(catalog)),
            Ok(false) => Ok(None),
            Err(_) => Ok(Some(catalog)),
        },
        None => Ok(None),
    }
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

fn cache_path() -> Result<PathBuf, String> {
    Ok(cache_dir()?.join(CACHE_FILE_NAME))
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
