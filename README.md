# RockGit

`RockGit` is a Rust desktop starter for a Git GUI client built with [`cacao`](https://github.com/ryanmcgrath/cacao) (native AppKit on macOS).

## What's included

- Native macOS window using `cacao` (AppKit)
- Repository and branch sidebars
- Mock commit graph list with colored lane glyphs
- Commit detail panel with fake changed-file metadata
- Search highlighting for commit rows
- MVC-style structure with shared UI actions and a central controller

## Project structure

```text
src/
├── main.rs                # thin application bootstrap
└── app/
	├── mod.rs             # app shell + action dispatch loop
	├── actions.rs         # reusable UI/application actions
	├── controller.rs      # state mutation logic
	├── model.rs           # application state + domain structs
	├── sample_data.rs     # mock repositories/branches/commits
	└── views/
		├── toolbar.rs
		├── sidebar.rs
		├── details.rs
		├── graph.rs
		├── rows.rs
		└── theme.rs
```

The UI emits `AppAction` values, and `AppController` is responsible for applying them to `AppModel`.

## Quick start

```bash
cargo run
```

## Why this stack

This starter uses `egui` because it is a strong fit for graph-heavy developer tools:

- fast UI iteration
- simple custom painting for DAG/graph rendering
- cross-platform desktop support
- pure Rust application stack

## Next steps

1. Replace `sample_commits()` with real Git history loading via `gix` or `git2`.
2. Move graph layout into a dedicated module.
3. Add background loading and repository watching.
4. Add file diff views and staging actions.


## Git client (`src/app/git/`)

A typed wrapper around the real `git` CLI, so SSH and HTTPS remotes use your own
`~/.ssh/config`, ssh-agent, credential helpers and `insteadOf` rules. Calls are blocking
(run them off the UI thread), non-interactive by default and time out via `GitOptions`.

```rust
let repo = GitClient::open("/path/to/repo")?;
let status = repo.status()?;
let rows = repo.graph(&LogOptions::default())?; // lane layout for the commit graph
```

Covers status, staging, commit, branches, remotes (fetch/pull/push), log, graph, diff and
hunk staging, merge, rebase, cherry-pick, revert, reset, stash, tags, blame, reflog,
worktrees, submodules and config. Run the tests with `cargo test git::`.
