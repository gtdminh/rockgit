use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::args::Args;
use super::error::{GitError, GitResult};

/// Settings applied to every `git` invocation.
#[derive(Clone, Debug)]
pub struct GitOptions {
    /// Path or name of the git executable.
    pub git_binary: PathBuf,
    /// Never prompt on a terminal (HTTPS credentials, editors). Recommended for GUI apps;
    /// credentials then come from credential helpers, the SSH agent or `askpass`.
    pub non_interactive: bool,
    /// Kill commands that run longer than this. `None` waits indefinitely.
    pub timeout: Option<Duration>,
    /// Overrides how SSH is spawned (`GIT_SSH_COMMAND`), e.g. `ssh -o BatchMode=yes -i ~/.ssh/key`.
    pub ssh_command: Option<String>,
    /// Program git calls to obtain passwords/passphrases (`GIT_ASKPASS`).
    pub askpass: Option<PathBuf>,
    /// Extra `-c key=value` settings, e.g. `credential.helper`.
    pub config: Vec<(String, String)>,
    /// Extra environment variables.
    pub env: Vec<(String, String)>,
}

impl Default for GitOptions {
    fn default() -> Self {
        Self {
            git_binary: PathBuf::from("git"),
            non_interactive: true,
            timeout: None,
            ssh_command: None,
            askpass: None,
            config: Vec::new(),
            env: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Output {
    /// `None` when the process was terminated by a signal.
    pub status: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl Output {
    pub fn success(&self) -> bool {
        self.status == Some(0)
    }

    pub fn stdout_str(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }

    pub fn stderr_str(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }
}

pub(crate) fn execute(
    opts: &GitOptions,
    dir: Option<&Path>,
    args: &Args,
    stdin: Option<Vec<u8>>,
    env: &[(&str, &str)],
) -> GitResult<Output> {
    let mut cmd = Command::new(&opts.git_binary);
    cmd.args(["-c", "core.quotepath=off"]);
    for (k, v) in &opts.config {
        cmd.arg("-c").arg(format!("{k}={v}"));
    }
    cmd.args(&args.0);
    if let Some(dir) = dir {
        cmd.current_dir(dir);
    }

    // Stable, parseable output and no editors blocking a GUI process.
    cmd.env("LC_ALL", "C")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_EDITOR", "true")
        .env("GIT_SEQUENCE_EDITOR", "true")
        .env("GIT_MERGE_AUTOEDIT", "no");
    if opts.non_interactive {
        cmd.env("GIT_TERMINAL_PROMPT", "0").env("GCM_INTERACTIVE", "never");
    }
    if let Some(ssh) = &opts.ssh_command {
        cmd.env("GIT_SSH_COMMAND", ssh);
    }
    if let Some(askpass) = &opts.askpass {
        cmd.env("GIT_ASKPASS", askpass);
    }
    for (k, v) in &opts.env {
        cmd.env(k, v);
    }
    for (k, v) in env {
        cmd.env(k, v);
    }

    cmd.stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            GitError::GitNotFound(e)
        } else {
            GitError::Io(e)
        }
    })?;

    if let (Some(data), Some(mut pipe)) = (stdin, child.stdin.take()) {
        thread::spawn(move || {
            let _ = pipe.write_all(&data);
        });
    }
    let mut out_pipe = child.stdout.take().expect("stdout is piped");
    let mut err_pipe = child.stderr.take().expect("stderr is piped");
    let out_reader = thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = out_pipe.read_to_end(&mut buf);
        buf
    });
    let err_reader = thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = err_pipe.read_to_end(&mut buf);
        buf
    });

    let status = match opts.timeout {
        None => child.wait()?,
        Some(limit) => {
            let deadline = Instant::now() + limit;
            loop {
                if let Some(status) = child.try_wait()? {
                    break status;
                }
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    // Reader threads are detached: a grandchild (ssh, remote helper) may still hold the pipes.
                    return Err(GitError::Timeout { command: args.describe() });
                }
                thread::sleep(Duration::from_millis(5));
            }
        },
    };

    Ok(Output {
        status: status.code(),
        stdout: out_reader.join().unwrap_or_default(),
        stderr: err_reader.join().unwrap_or_default(),
    })
}
