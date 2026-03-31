use std::env;
use std::fs;
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
    pub items: Vec<Package>,
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
    items.sort_by(|left, right| left.token.cmp(&right.token));

    let catalog = Catalog {
        generated_at: now_unix_timestamp(),
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

    let serialized = serde_json::to_vec(&catalog)
        .map_err(|error| format!("Failed to serialize the package catalog: {error}"))?;

    let temp_path = cache_path.with_extension("tmp");
    fs::write(&temp_path, serialized).map_err(|error| {
        format!(
            "Failed to write cache file {}: {error}",
            temp_path.display()
        )
    })?;
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

    read_cache_any(path)
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
    let base_dir = if let Ok(path) = env::var("XDG_CACHE_HOME") {
        PathBuf::from(path)
    } else if cfg!(target_os = "macos") {
        home_dir()?.join("Library").join("Caches")
    } else {
        home_dir()?.join(".cache")
    };

    Ok(base_dir.join("brewfind").join(CACHE_FILE_NAME))
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
