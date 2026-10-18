# RockGit

`RockGit` is a native macOS Git client prototype written in Rust with
[`cacao`](https://github.com/ryanmcgrath/cacao), using AppKit for the desktop UI and the
installed `git` CLI for repository operations.

The app starts with realistic sample data so the UI is immediately explorable, then can load a
real repository and replace the mock graph with live Git history.

## Status

- macOS-only UI (`cacao` + AppKit)
- Rust 2024 edition
- Real Git integration is available through `src/app/git/`
- Main desktop flow is implemented for browsing repositories, branches, commits, and commit files

## What the app currently does

- Shows a native macOS window with repository, branch, commit, and details panes
- Starts with sample repositories, branches, and a multi-lane commit graph
- Opens a real Git repository and loads:
  - branches
  - tags
  - commit graph/history
  - changed files for the selected commit
- Supports commit search highlighting by subject, author, short SHA, and refs
- Supports branch filtering and toggling remote branch visibility
- Supports keyboard navigation between newer/older commits
- Wires toolbar actions for fetch, pull, push, and refresh through a background Git service

## Architecture overview

The app follows a small MVC-style structure:

- `AppAction` represents UI and background-service events
- `AppController` mutates `AppModel`
- `git_service` performs blocking Git work on a background thread
- `views/*` render the AppKit UI

The desktop entry point is `src/main.rs`, which calls `app::run()`.

## Project structure

```text
src/
├── main.rs                    # thin application bootstrap
└── app/
	├── mod.rs                 # app shell, window setup, dispatch loop
	├── actions.rs             # UI and service actions
	├── bus.rs                 # app-wide event bus
	├── controller.rs          # state mutation logic
	├── git_service.rs         # background Git loading/service thread
	├── model.rs               # application state + domain structs
	├── sample_data.rs         # mock repositories/branches/commits
	├── git/                   # typed wrapper around the git CLI
	│   ├── client.rs
	│   ├── graph.rs
	│   ├── history.rs
	│   ├── remote.rs
	│   ├── staging.rs
	│   ├── status.rs
	│   ├── tag.rs
	│   ├── tests.rs
	│   └── ...
	└── views/
		├── details.rs         # selected commit file list
		├── graph.rs           # commit list + graph rendering helpers
		├── header.rs          # commit table header
		├── rows.rs            # shared row rendering helpers
		├── sidebar.rs         # repository/branch lists
		├── theme.rs           # colors/fonts
		└── toolbar.rs         # fetch/pull/push/search/filter controls
```

## Requirements

- macOS
- Rust toolchain with Cargo
- `git` available in your shell environment

Because the UI is built on AppKit, this project is not intended to run cross-platform in its
current form.

## Quick start

Run the desktop app:

```bash
cargo run
```

Run the Git client test suite:

```bash
cargo test git::
```

## Sample data vs real repositories

On startup, `AppModel::new()` seeds the interface with sample repositories, branches, and commit
history from `src/app/sample_data.rs`.

When a real repository is opened, `git_service` uses `GitClient` to load live repository data and
replaces the sample content with:

- branch and tag lists
- commit graph rows
- current branch selection
- changed files for the selected commit

The current implementation loads up to 1000 commits per repository refresh.

## Git client (`src/app/git/`)

`src/app/git/` is a typed wrapper around the real `git` executable instead of `libgit2`. That
means SSH and HTTPS remotes behave like they do in the terminal, including your:

- `~/.ssh/config`
- SSH agent
- credential helpers
- proxy settings
- `url.*.insteadOf` rewrites

Git calls are blocking, so the UI routes repository loading through a background thread.

```rust,ignore
let repo = GitClient::open("/path/to/repo")?;
let status = repo.status(&StatusOptions::default())?;
let rows = repo.graph(&LogOptions::default())?;
```

The module includes support for status, staging, commits, branches, remotes, log/history, graph
layout, diff and hunk staging, merge, rebase, cherry-pick, revert, reset, stash, tags, blame,
reflog, worktrees, submodules, and config helpers.

## Verified test status

The Git module tests currently pass locally with:

```bash
cargo test git:: -- --nocapture
```

## Near-term improvement ideas

1. Wire more toolbar and context actions into the UI flow.
2. Add richer diff and staging views in the desktop interface.
3. Add repository watching and more granular background refresh.
4. Improve error presentation for repository load and Git operation failures.
