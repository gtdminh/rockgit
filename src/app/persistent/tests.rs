use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_path() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("rockgit-persistent-tests-{stamp}"))
}

#[test]
fn missing_recents_file_returns_empty_list() {
    let path = unique_path().join("recents");

    let repositories = load_recent_repositories_from_path(&path).unwrap();

    assert!(repositories.is_empty());
}

#[test]
fn save_and_load_round_trip_preserves_order() {
    let root = unique_path();
    let path = root.join("recents");
    let repositories = vec![
        Repository {
            name: "repo-b".to_owned(),
            path: "/tmp/repo-b".to_owned(),
            branch_count: 3,
            has_uncommitted_changes: true,
        },
        Repository {
            name: "repo-a".to_owned(),
            path: "/tmp/repo-a".to_owned(),
            branch_count: 1,
            has_uncommitted_changes: false,
        },
    ];

    save_recent_repositories_to_path(&path, &repositories).unwrap();
    let loaded = load_recent_repositories_from_path(&path).unwrap();

    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].path, "/tmp/repo-b");
    assert_eq!(loaded[0].name, "repo-b");
    assert_eq!(loaded[0].branch_count, 0);
    assert!(!loaded[0].has_uncommitted_changes);
    assert_eq!(loaded[1].path, "/tmp/repo-a");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn missing_config_file_returns_none_for_path_loader() {
    let path = unique_path().join("config");

    let config = load_config_from_path(&path).unwrap();

    assert_eq!(config, None);
}

#[test]
fn save_and_load_config_round_trip() {
    let root = unique_path();
    let path = root.join("config");
    let config = AppConfig {
        autorefresh_time: 30,
        ai: crate::app::model::AiConfig {
            provider_cli: AiProviderCli::Copilot,
            cli_path: "/usr/local/bin/copilot cli".to_owned(),
        },
    };

    save_config_to_path(&path, &config).unwrap();
    let loaded = load_config_from_path(&path).unwrap().unwrap();

    assert_eq!(loaded, config);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn config_round_trip_escapes_cli_path() {
    let root = unique_path();
    let path = root.join("config");
    let config = AppConfig {
        autorefresh_time: 0,
        ai: crate::app::model::AiConfig {
            provider_cli: AiProviderCli::Gemini,
            cli_path: r#"/opt/bin/cli with \"quotes\" and \\slashes"#.to_owned(),
        },
    };

    save_config_to_path(&path, &config).unwrap();
    let loaded = load_config_from_path(&path).unwrap().unwrap();

    assert_eq!(loaded.ai.cli_path, config.ai.cli_path);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn invalid_provider_is_rejected() {
    let root = unique_path();
    let path = root.join("config");
    fs::create_dir_all(&root).unwrap();
    fs::write(&path, "provider = \"unknown\"\n").unwrap();

    let err = load_config_from_path(&path).unwrap_err();

    assert!(
        err.contains("unknown config key") || err.contains("invalid ai.provider_cli")
    );
    let _ = fs::remove_dir_all(root);
}
