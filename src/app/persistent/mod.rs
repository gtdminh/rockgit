use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::app::model::{AiProviderCli, AppConfig, Repository};

const ROCKGIT_DIR: &str = ".rockgit";
const RECENTS_FILE: &str = "recents";
const CONFIG_FILE: &str = "config";

/// Loads the persisted recent repositories from `~/.rockgit/recents`.
pub fn load_recent_repositories() -> Result<Vec<Repository>, String> {
    load_recent_repositories_from_path(&recents_file_path()?)
}

/// Persists recent repositories to `~/.rockgit/recents` in MRU order.
pub fn save_recent_repositories(repositories: &[Repository]) -> Result<(), String> {
    save_recent_repositories_to_path(&recents_file_path()?, repositories)
}

/// Loads app config from `~/.rockgit/config`, creating it with defaults if it does not exist.
pub fn load_config() -> Result<AppConfig, String> {
    match load_config_from_path(&config_file_path()?)? {
        Some(config) => Ok(config),
        None => {
            let config = AppConfig::default();
            save_config(&config)?;
            Ok(config)
        }
    }
}

/// Persists app config to `~/.rockgit/config`.
pub fn save_config(config: &AppConfig) -> Result<(), String> {
    save_config_to_path(&config_file_path()?, config)
}

fn recents_file_path() -> Result<PathBuf, String> {
    Ok(rockgit_dir_path()?.join(RECENTS_FILE))
}

fn config_file_path() -> Result<PathBuf, String> {
    Ok(rockgit_dir_path()?.join(CONFIG_FILE))
}

fn rockgit_dir_path() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME")
        .ok_or_else(|| "HOME is not set, so RockGit settings cannot be stored".to_owned())?;
    Ok(PathBuf::from(home).join(ROCKGIT_DIR))
}

pub(crate) fn load_recent_repositories_from_path(path: &Path) -> Result<Vec<Repository>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;

    Ok(contents
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(repository_from_path)
        .collect())
}

pub(crate) fn save_recent_repositories_to_path(
    path: &Path,
    repositories: &[Repository],
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }

    let mut contents = repositories
        .iter()
        .map(|repository| repository.path.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    if !contents.is_empty() {
        contents.push('\n');
    }

    fs::write(path, contents)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn repository_from_path(path: &str) -> Repository {
    let name = Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| path.to_owned());

    Repository {
        name,
        path: path.to_owned(),
        branch_count: 0,
        has_uncommitted_changes: false,
    }
}

pub(crate) fn load_config_from_path(path: &Path) -> Result<Option<AppConfig>, String> {
    if !path.exists() {
        return Ok(None);
    }

    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;

    parse_config(&contents).map(Some)
}

pub(crate) fn save_config_to_path(path: &Path, config: &AppConfig) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }

    fs::write(path, format_config(config))
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn parse_config(contents: &str) -> Result<AppConfig, String> {
    let mut config = AppConfig::default();
    let mut in_ai_section = false;

    for (line_number, raw_line) in contents.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.starts_with('[') {
            match line {
                "[ai]" => {
                    in_ai_section = true;
                    continue;
                }
                _ => {
                    return Err(format!(
                        "invalid config section on line {}: {line}",
                        line_number + 1
                    ));
                }
            }
        }

        let Some((key, value)) = line.split_once('=') else {
            return Err(format!(
                "invalid config entry on line {}: {line}",
                line_number + 1
            ));
        };

        let key = key.trim();
        let value = unquote(value.trim());

        if in_ai_section {
            match key {
                "provider_cli" => {
                    config.ai.provider_cli = AiProviderCli::parse(&value).ok_or_else(|| {
                        format!(
                            "invalid ai.provider_cli on line {}: {}",
                            line_number + 1,
                            value
                        )
                    })?;
                }
                "cli_path" => config.ai.cli_path = value,
                _ => {
                    return Err(format!(
                        "unknown ai config key on line {}: {key}",
                        line_number + 1
                    ));
                }
            }
        } else {
            match key {
                "autorefresh_time" => {
                    config.autorefresh_time = value.parse::<u64>().map_err(|error| {
                        format!(
                            "invalid autorefresh_time on line {}: {error}",
                            line_number + 1
                        )
                    })?;
                }
                _ => {
                    return Err(format!(
                        "unknown config key on line {}: {key}",
                        line_number + 1
                    ));
                }
            }
        }
    }

    Ok(config)
}

fn format_config(config: &AppConfig) -> String {
    format!(
        "autorefresh_time = {}\n\n[ai]\nprovider_cli = \"{}\"\ncli_path = \"{}\"\n",
        config.autorefresh_time,
        config.ai.provider_cli.as_str(),
        escape_config_string(&config.ai.cli_path)
    )
}

fn unquote(value: &str) -> String {
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        value[1..value.len() - 1]
            .replace(r#"\\"#, "\\")
            .replace(r#"\""#, "\"")
    } else {
        value.to_owned()
    }
}

fn escape_config_string(value: &str) -> String {
    value.replace('\\', r#"\\"#).replace('"', r#"\""#)
}

#[cfg(test)]
mod tests;
