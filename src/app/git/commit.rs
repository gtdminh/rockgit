use super::args::Args;
use super::client::GitClient;
use super::error::{GitError, GitResult};

#[derive(Clone, Debug, Default)]
pub struct CommitOptions {
    /// Full message (subject, blank line, body). May be empty only with `amend`.
    pub message: String,
    /// Replace the last commit instead of adding one. An empty message keeps the old one.
    pub amend: bool,
    /// Stage all modified/deleted tracked files first (`-a`).
    pub all: bool,
    pub allow_empty: bool,
    pub signoff: bool,
    /// Skip pre-commit and commit-msg hooks.
    pub no_verify: bool,
    /// GPG/SSH-sign the commit.
    pub sign: bool,
    /// Override the author, as `Name <email>`.
    pub author: Option<String>,
    /// Commit only these paths, ignoring other staged changes.
    pub paths: Vec<String>,
}

impl GitClient {
    /// Creates a commit from the index and returns its id.
    pub fn commit(&self, o: &CommitOptions) -> GitResult<String> {
        let has_message = !o.message.trim().is_empty();
        if !has_message && !o.amend {
            return Err(GitError::InvalidInput("commit message is empty".into()));
        }
        let args = Args::new("commit")
            .flag(o.amend, "--amend")
            .flag(o.all, "-a")
            .flag(o.allow_empty, "--allow-empty")
            .flag(o.signoff, "--signoff")
            .flag(o.no_verify, "--no-verify")
            .flag(o.sign, "--gpg-sign")
            .flag(!has_message, "--no-edit")
            .flag(has_message, "--file=-")
            .kv("author", o.author.as_deref())
            .paths(&o.paths);
        self.run_stdin(args, o.message.as_bytes())?;
        self.run_str(Args::new("rev-parse").arg("HEAD"))
    }

    /// Changes only the message of the last commit.
    pub fn reword_last_commit(&self, message: &str) -> GitResult<String> {
        self.commit(&CommitOptions {
            message: message.to_string(),
            amend: true,
            ..Default::default()
        })
    }
}
