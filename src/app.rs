use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::symlink;

pub const DEFAULT_BINARY_NAME: &str = "brau";
pub const ALIAS_BINARY_NAME: &str = "bro";
pub const EASTER_EGG_COMMAND: &str = "hidden-easter-egg";

#[derive(Debug, PartialEq, Eq)]
pub enum BroAliasStatus {
    Created(PathBuf),
    AlreadyAvailable(PathBuf),
}

pub fn display_name() -> String {
    match invoked_binary_name().as_deref() {
        Some(ALIAS_BINARY_NAME) => ALIAS_BINARY_NAME.to_string(),
        _ => DEFAULT_BINARY_NAME.to_string(),
    }
}

pub fn is_hidden_easter_egg_command(args: &[String]) -> bool {
    args.len() == 1 && args[0] == EASTER_EGG_COMMAND
}

pub fn unlock_bro_alias() -> Result<BroAliasStatus, String> {
    let invoked = env::args_os()
        .next()
        .ok_or_else(|| "Could not determine how brau was launched.".to_string())?;
    let executable_path = locate_invoked_executable(&invoked)?;
    create_bro_alias_for(&executable_path)
}

fn invoked_binary_name() -> Option<String> {
    env::args_os().next().and_then(|path| {
        Path::new(&path)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
    })
}

fn locate_invoked_executable(invoked: &OsStr) -> Result<PathBuf, String> {
    let invoked_path = Path::new(invoked);
    if invoked_path.components().count() > 1 {
        return resolve_invoked_path(invoked_path);
    }

    let path = env::var_os("PATH")
        .ok_or_else(|| format!("Could not find `{DEFAULT_BINARY_NAME}` in PATH."))?;

    for directory in env::split_paths(&path) {
        let candidate = directory.join(invoked_path);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    Err(format!(
        "Could not find `{}` in PATH to unlock `{ALIAS_BINARY_NAME}`.",
        invoked_path.display()
    ))
}

fn resolve_invoked_path(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }

    env::current_dir()
        .map(|cwd| cwd.join(path))
        .map_err(|error| format!("Failed to resolve the executable path: {error}"))
}

fn create_bro_alias_for(executable_path: &Path) -> Result<BroAliasStatus, String> {
    let executable_name = executable_path.file_name().ok_or_else(|| {
        format!(
            "Could not determine the executable name for {}.",
            executable_path.display()
        )
    })?;
    let alias_path = executable_path.with_file_name(ALIAS_BINARY_NAME);
    let display_path = normalized_display_path(&alias_path);

    if executable_name == OsStr::new(ALIAS_BINARY_NAME) {
        return Ok(BroAliasStatus::AlreadyAvailable(display_path));
    }

    if fs::symlink_metadata(&alias_path).is_ok() {
        if alias_points_to_same_executable(&alias_path, executable_path)? {
            return Ok(BroAliasStatus::AlreadyAvailable(display_path));
        }

        return Err(format!(
            "Something is already squatting the `{ALIAS_BINARY_NAME}` name at {}. Evict it first, then we can talk shades.",
            alias_path.display()
        ));
    }

    let link_target = Path::new(executable_name);

    #[cfg(unix)]
    {
        symlink(link_target, &alias_path).map_err(|error| {
            format!(
                "Failed to create `{ALIAS_BINARY_NAME}` at {}: {error}",
                alias_path.display()
            )
        })?;
        Ok(BroAliasStatus::Created(display_path))
    }

    #[cfg(not(unix))]
    {
        let _ = link_target;
        Err("The `bro` alias only works on Unix-like systems. Windows gets no shades. Those are the rules.".to_string())
    }
}

fn normalized_display_path(path: &Path) -> PathBuf {
    let Some(file_name) = path.file_name() else {
        return path.to_path_buf();
    };

    match path.parent() {
        Some(parent) => match fs::canonicalize(parent) {
            Ok(parent) => parent.join(file_name),
            Err(_) => path.to_path_buf(),
        },
        None => path.to_path_buf(),
    }
}

fn alias_points_to_same_executable(
    alias_path: &Path,
    executable_path: &Path,
) -> Result<bool, String> {
    let alias_target = match fs::canonicalize(alias_path) {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(format!(
                "Failed to inspect the existing `{ALIAS_BINARY_NAME}` alias at {}: {error}",
                alias_path.display()
            ))
        }
    };
    let executable_target = fs::canonicalize(executable_path).map_err(|error| {
        format!(
            "Failed to inspect the executable at {}: {error}",
            executable_path.display()
        )
    })?;

    Ok(alias_target == executable_target)
}

#[cfg(test)]
mod tests {
    use super::{
        create_bro_alias_for, is_hidden_easter_egg_command, normalized_display_path, BroAliasStatus,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(label: &str) -> Self {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should move forward")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "brau-app-test-{label}-{}-{timestamp}",
                process::id()
            ));
            fs::create_dir_all(&path).expect("test temp dir should be created");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn hidden_easter_egg_requires_exact_single_argument() {
        assert!(is_hidden_easter_egg_command(&[
            "hidden-easter-egg".to_string()
        ]));
        assert!(!is_hidden_easter_egg_command(&[
            "hidden-easter-egg".to_string(),
            "--help".to_string()
        ]));
        assert!(!is_hidden_easter_egg_command(&["install".to_string()]));
    }

    #[cfg(unix)]
    #[test]
    fn creates_bro_alias_next_to_brau() {
        let temp_dir = TestDir::new("create");
        let executable_path = temp_dir.path().join("brau");
        fs::write(&executable_path, "brau").expect("executable placeholder should be written");

        let status = create_bro_alias_for(&executable_path).expect("alias should be created");
        assert_eq!(
            status,
            BroAliasStatus::Created(normalized_display_path(&temp_dir.path().join("bro")))
        );
        assert_eq!(
            fs::read_link(temp_dir.path().join("bro")).expect("symlink target should be readable"),
            PathBuf::from("brau")
        );
    }

    #[cfg(unix)]
    #[test]
    fn reuses_existing_alias_when_it_points_to_the_same_binary() {
        let temp_dir = TestDir::new("existing");
        let executable_path = temp_dir.path().join("brau");
        let alias_path = temp_dir.path().join("bro");
        fs::write(&executable_path, "brau").expect("executable placeholder should be written");
        symlink("brau", &alias_path).expect("alias should be created");

        let status = create_bro_alias_for(&executable_path).expect("alias should already exist");
        assert_eq!(
            status,
            BroAliasStatus::AlreadyAvailable(normalized_display_path(&alias_path))
        );
    }

    #[cfg(unix)]
    #[test]
    fn refuses_to_overwrite_existing_bro_file() {
        let temp_dir = TestDir::new("conflict");
        let executable_path = temp_dir.path().join("brau");
        let alias_path = temp_dir.path().join("bro");
        fs::write(&executable_path, "brau").expect("executable placeholder should be written");
        fs::write(&alias_path, "occupied").expect("conflicting file should be written");

        let error = create_bro_alias_for(&executable_path)
            .expect_err("conflicting bro file should be rejected");
        assert!(error.contains("already exists"));
    }
}
