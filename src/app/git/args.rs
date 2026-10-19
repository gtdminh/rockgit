use std::ffi::{OsStr, OsString};
use std::fmt::Display;

use super::error::redact;

/// Builder for a `git` argument list (excluding the `git` binary itself).
#[derive(Clone, Debug, Default)]
pub(crate) struct Args(pub(crate) Vec<OsString>);

impl Args {
    pub(crate) fn new(command: &str) -> Self {
        Self(vec![command.into()])
    }

    pub(crate) fn arg(mut self, a: impl AsRef<OsStr>) -> Self {
        self.0.push(a.as_ref().to_owned());
        self
    }

    pub(crate) fn args<I, S>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.0
            .extend(items.into_iter().map(|s| s.as_ref().to_owned()));
        self
    }

    pub(crate) fn flag(self, on: bool, flag: &str) -> Self {
        if on { self.arg(flag) } else { self }
    }

    /// Appends the value only when present.
    pub(crate) fn opt<S: AsRef<OsStr>>(self, value: Option<S>) -> Self {
        match value {
            Some(v) => self.arg(v),
            None => self,
        }
    }

    /// Appends `--name=value` only when present.
    pub(crate) fn kv<D: Display>(self, name: &str, value: Option<D>) -> Self {
        match value {
            Some(v) => self.arg(format!("--{name}={v}")),
            None => self,
        }
    }

    /// Appends `--` followed by the paths; no-op when there are none.
    pub(crate) fn paths<I, S>(self, items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut items = items.into_iter().peekable();
        if items.peek().is_none() {
            return self;
        }
        self.arg("--").args(items)
    }

    pub(crate) fn describe(&self) -> String {
        let joined = self
            .0
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ");
        redact(&format!("git {joined}"))
    }
}
