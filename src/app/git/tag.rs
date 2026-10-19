use super::args::Args;
use super::client::{GitClient, safe_ref};
use super::error::GitResult;
use super::types::Tag;

impl GitClient {
    pub fn tags(&self) -> GitResult<Vec<Tag>> {
        let args = Args::new("for-each-ref")
            .arg("--format=%(refname:short)%1f%(objecttype)%1f%(objectname)%1f%(*objectname)%1f%(contents:subject)")
            .arg("--sort=-creatordate")
            .arg("refs/tags");
        let out = self.run(args)?.stdout_str();
        Ok(out
            .lines()
            .filter_map(|line| {
                let f: Vec<&str> = line.splitn(5, '\x1f').collect();
                if f.len() < 5 {
                    return None;
                }
                let annotated = f[1] == "tag";
                Some(Tag {
                    name: f[0].to_string(),
                    target: if annotated { f[3] } else { f[2] }.to_string(),
                    annotated,
                    subject: f[4].to_string(),
                })
            })
            .collect())
    }

    /// Creates a tag at `target` (default HEAD). A `message` makes it an annotated tag.
    pub fn create_tag(
        &self,
        name: &str,
        target: Option<&str>,
        message: Option<&str>,
        force: bool,
    ) -> GitResult<()> {
        safe_ref(name)?;
        if let Some(t) = target {
            safe_ref(t)?;
        }
        let args = Args::new("tag")
            .flag(force, "--force")
            .flag(message.is_some(), "--annotate")
            .flag(message.is_some(), "--file=-")
            .arg(name)
            .opt(target);
        self.run_stdin(args, message.unwrap_or_default().as_bytes())
            .map(drop)
    }

    pub fn delete_tag(&self, name: &str) -> GitResult<()> {
        safe_ref(name)?;
        self.run(Args::new("tag").args(["--delete", name]))
            .map(drop)
    }

    /// Pushes one tag to a remote.
    pub fn push_tag(&self, remote: &str, name: &str) -> GitResult<()> {
        safe_ref(remote)?;
        safe_ref(name)?;
        self.run(
            Args::new("push")
                .arg(remote)
                .arg(format!("refs/tags/{name}")),
        )
        .map(drop)
    }

    /// Deletes a tag on a remote.
    pub fn delete_remote_tag(&self, remote: &str, name: &str) -> GitResult<()> {
        safe_ref(remote)?;
        safe_ref(name)?;
        self.run(
            Args::new("push")
                .arg(remote)
                .arg(format!(":refs/tags/{name}")),
        )
        .map(drop)
    }
}
