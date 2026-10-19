use std::fmt;
use std::io;
use std::path::PathBuf;

pub type GitResult<T> = Result<T, GitError>;

#[derive(Debug)]
pub enum GitError {
    /// The `git` executable could not be started.
    GitNotFound(io::Error),
    Io(io::Error),
    NotARepository(PathBuf),
    /// `git` ran but exited unsuccessfully. `command` and `stderr` have credentials redacted.
    Command {
        command: String,
        code: Option<i32>,
        stderr: String,
    },
    Timeout {
        command: String,
    },
    Parse(String),
    InvalidInput(String),
}

impl GitError {
    pub fn stderr(&self) -> Option<&str> {
        match self {
            GitError::Command { stderr, .. } => Some(stderr),
            _ => None,
        }
    }

    /// True when a remote operation failed because credentials were missing or rejected.
    pub fn is_auth_failure(&self) -> bool {
        const MARKERS: [&str; 7] = [
            "Authentication failed",
            "Permission denied (publickey",
            "could not read Username",
            "could not read Password",
            "terminal prompts disabled",
            "Host key verification failed",
            "HTTP Basic: Access denied",
        ];
        self.stderr()
            .is_some_and(|s| MARKERS.iter().any(|m| s.contains(m)))
    }

    pub fn is_conflict(&self) -> bool {
        self.stderr()
            .is_some_and(|s| s.contains("CONFLICT") || s.contains("conflict"))
    }
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GitError::GitNotFound(e) => write!(f, "git executable not found: {e}"),
            GitError::Io(e) => write!(f, "I/O error: {e}"),
            GitError::NotARepository(p) => write!(f, "not a git repository: {}", p.display()),
            GitError::Command {
                command,
                code,
                stderr,
            } => {
                let code = code.map_or("signal".to_string(), |c| c.to_string());
                write!(f, "`{command}` failed (exit {code}): {}", stderr.trim())
            }
            GitError::Timeout { command } => write!(f, "`{command}` timed out"),
            GitError::Parse(m) => write!(f, "could not parse git output: {m}"),
            GitError::InvalidInput(m) => write!(f, "invalid input: {m}"),
        }
    }
}

impl std::error::Error for GitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            GitError::GitNotFound(e) | GitError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for GitError {
    fn from(e: io::Error) -> Self {
        GitError::Io(e)
    }
}

/// Replaces `user:password@` in URLs so secrets never reach logs or error messages.
pub(crate) fn redact(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find("://") {
        let (head, tail) = rest.split_at(i + 3);
        out.push_str(head);
        let end = tail
            .find(|c: char| c == '/' || c.is_whitespace())
            .unwrap_or(tail.len());
        let (authority, remainder) = tail.split_at(end);
        match authority.rfind('@') {
            Some(at) => {
                out.push_str("***@");
                out.push_str(&authority[at + 1..]);
            }
            None => out.push_str(authority),
        }
        rest = remainder;
    }
    out.push_str(rest);
    out
}
