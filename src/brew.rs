use std::collections::HashSet;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;

use crate::catalog;
use crate::motion::{should_run_motion, MotionSettings};

const BREW_COMMAND_CACHE_FILE: &str = "brew-commands-v1.txt";
const KNOWN_BREW_COMMANDS: &[&str] = &[
    "--config",
    "--cache",
    "--caskroom",
    "--cellar",
    "--env",
    "--prefix",
    "--repo",
    "--repository",
    "--taps",
    "--version",
    "-S",
    "-v",
    "abv",
    "alias",
    "analytics",
    "audit",
    "autoremove",
    "bottle",
    "bump",
    "bump-cask-pr",
    "bump-formula-pr",
    "bump-revision",
    "bump-unversioned-casks",
    "bundle",
    "casks",
    "cat",
    "cleanup",
    "command",
    "command-not-found-init",
    "commands",
    "completions",
    "config",
    "contributions",
    "create",
    "debugger",
    "deps",
    "desc",
    "determine-test-runners",
    "developer",
    "dispatch-build-bottle",
    "docs",
    "doctor",
    "edit",
    "extract",
    "fetch",
    "formula",
    "formula-analytics",
    "formulae",
    "dr",
    "environment",
    "generate-analytics-api",
    "generate-cask-api",
    "generate-cask-ci-matrix",
    "generate-formula-api",
    "generate-man-completions",
    "generate-zap",
    "gist-logs",
    "help",
    "home",
    "homepage",
    "info",
    "instal",
    "install",
    "install-bundler-gems",
    "irb",
    "lc",
    "leaves",
    "lgtm",
    "ln",
    "link",
    "linkage",
    "list",
    "livecheck",
    "log",
    "ls",
    "mcp-server",
    "migrate",
    "missing",
    "nodenv-sync",
    "options",
    "outdated",
    "pin",
    "postinstall",
    "post_install",
    "pr-automerge",
    "pr-publish",
    "pr-pull",
    "pr-upload",
    "prof",
    "pyenv-sync",
    "rbenv-sync",
    "readall",
    "reinstall",
    "release",
    "remove",
    "rm",
    "rubocop",
    "ruby",
    "rubydoc",
    "search",
    "services",
    "setup-ruby",
    "sh",
    "shellenv",
    "source",
    "style",
    "tab",
    "tap",
    "tap-info",
    "tap-new",
    "tc",
    "test",
    "test-bot",
    "tests",
    "typecheck",
    "unalias",
    "unbottled",
    "uninstal",
    "uninstall",
    "unlink",
    "unpack",
    "unpin",
    "untap",
    "up",
    "update",
    "update-if-needed",
    "update-license-data",
    "update-maintainers",
    "update-perl-resources",
    "update-python-resources",
    "update-report",
    "update-reset",
    "update-sponsors",
    "update-test",
    "upgrade",
    "uses",
    "vendor-gems",
    "vendor-install",
    "verify",
    "version-install",
    "which-formula",
    "which-update",
];

#[derive(Clone, Copy)]
enum OutputStream {
    Stdout,
    Stderr,
}

struct OutputChunk {
    stream: OutputStream,
    bytes: Vec<u8>,
}

pub fn brew_command_display(args: &[String]) -> String {
    if args.is_empty() {
        "brew".to_string()
    } else {
        format!("brew {}", args.join(" "))
    }
}

pub fn run_brew_command<A>(
    args: &[String],
    motion: MotionSettings,
    animate: A,
) -> Result<ExitStatus, String>
where
    A: FnOnce(),
{
    if !should_run_motion(motion) {
        return Command::new("brew")
            .args(args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|error| format!("Failed to launch Homebrew: {error}"));
    }

    let mut child = Command::new("brew")
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Failed to launch Homebrew: {error}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture Homebrew stdout.".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to capture Homebrew stderr.".to_string())?;

    let (sender, receiver) = mpsc::channel();
    let stdout_reader = spawn_output_reader(stdout, OutputStream::Stdout, sender.clone());
    let stderr_reader = spawn_output_reader(stderr, OutputStream::Stderr, sender);

    animate();
    relay_output(receiver)?;
    let status = child
        .wait()
        .map_err(|error| format!("Failed to wait for Homebrew: {error}"))?;

    join_output_reader(stdout_reader, "stdout")?;
    join_output_reader(stderr_reader, "stderr")?;

    Ok(status)
}

pub fn refresh_brew_command_cache() -> Result<(), String> {
    let output = Command::new("brew")
        .args(["commands", "--quiet", "--include-aliases"])
        .output()
        .map_err(|error| format!("Failed to ask Homebrew for its command list: {error}"))?;

    if !output.status.success() {
        return Err(format!(
            "Homebrew failed while listing commands with status {}.",
            output.status
        ));
    }

    let commands = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    let cache_path = brew_command_cache_path()?;
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Failed to create command cache directory {}: {error}",
                parent.display()
            )
        })?;
    }

    fs::write(&cache_path, format!("{commands}\n")).map_err(|error| {
        format!(
            "Failed to write brew command cache {}: {error}",
            cache_path.display()
        )
    })?;

    Ok(())
}

pub fn is_known_brew_command(command: &str) -> bool {
    KNOWN_BREW_COMMANDS.contains(&command) || cached_brew_commands().contains(command)
}

fn spawn_output_reader<R>(
    mut reader: R,
    stream: OutputStream,
    sender: mpsc::Sender<OutputChunk>,
) -> thread::JoinHandle<io::Result<()>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut buffer = [0u8; 8192];

        loop {
            let read = reader.read(&mut buffer)?;
            if read == 0 {
                return Ok(());
            }

            if sender
                .send(OutputChunk {
                    stream,
                    bytes: buffer[..read].to_vec(),
                })
                .is_err()
            {
                return Ok(());
            }
        }
    })
}

fn relay_output(receiver: mpsc::Receiver<OutputChunk>) -> Result<(), String> {
    while let Ok(chunk) = receiver.recv() {
        match chunk.stream {
            OutputStream::Stdout => {
                let mut stdout = io::stdout();
                stdout
                    .write_all(&chunk.bytes)
                    .map_err(|error| format!("Failed to forward Homebrew stdout: {error}"))?;
                stdout
                    .flush()
                    .map_err(|error| format!("Failed to flush Homebrew stdout: {error}"))?;
            }
            OutputStream::Stderr => {
                let mut stderr = io::stderr();
                stderr
                    .write_all(&chunk.bytes)
                    .map_err(|error| format!("Failed to forward Homebrew stderr: {error}"))?;
                stderr
                    .flush()
                    .map_err(|error| format!("Failed to flush Homebrew stderr: {error}"))?;
            }
        }
    }

    Ok(())
}

fn join_output_reader(
    handle: thread::JoinHandle<io::Result<()>>,
    label: &str,
) -> Result<(), String> {
    handle
        .join()
        .map_err(|_| format!("The Homebrew {label} reader stopped unexpectedly."))?
        .map_err(|error| format!("Failed to read Homebrew {label}: {error}"))
}

fn cached_brew_commands() -> HashSet<String> {
    let cache_path = match brew_command_cache_path() {
        Ok(path) => path,
        Err(_) => return HashSet::new(),
    };

    let contents = match fs::read_to_string(&cache_path) {
        Ok(contents) => contents,
        Err(_) => return HashSet::new(),
    };

    contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn brew_command_cache_path() -> Result<PathBuf, String> {
    Ok(catalog::cache_dir()?.join(BREW_COMMAND_CACHE_FILE))
}
