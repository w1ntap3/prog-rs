# AGENTS.md

## Project

prog-rs — a [GPUI](https://gpui.rs) (Zed's GPU-accelerated UI framework) app.
Currently a maximized shell with a left menu (sections: main, finances,
appeal; "main" is the default page) and placeholder content; installs on
PATH as `prog-rs`.

## Commands

- Build: `cargo build --release`
- Run: `cargo run`
- Test: `cargo test`
- Install on PATH: `cargo install --path .` (installs to `~/.cargo/bin/prog-rs`)
- Install KDE launcher (KRunner entry + icon): `scripts/install-desktop-entry.sh`

## Workflow rules

- **Always use SSH for GitHub, never HTTPS** (remote, git deps, clones alike).
- **Never commit or push without explicit user approval.** Repo:
  `git@github.com:w1ntap3/prog-rs.git`.
- **Commit messages: `prefix: one concise sentence`** describing what was
  done to the app. Prefix is a type (`feat`, `bugfix`, `perf`, `refactor`,
  `docs`, `chore`) optionally scoped to a module (`config/feat:`,
  `app/bugfix:`, ...) when the change centers on one. A slightly longer
  sentence is fine for big updates; never bullet essays or bodies.
- **Before every push, re-read the Architecture section of this file against
  the actual tree** — document new modules and decisions, drop stale ones.
  The push gate is only satisfied when the docs describe the app as it is.
- **Personal data never gets committed.** User data (finances etc.) lives in
  `$XDG_DATA_HOME/prog-rs` (see `src/data.rs`); the repo contains structure
  and code only. Configs shared for import/export are structure-only too.

## Code principles

- **Structure for reversibility.** Small, focused modules (see `src/`): each
  decision should be replaceable in one place without touching the rest.
  Prefer many small units over one clever big one.
- **Speed yes, spaghetti no.** Only optimize when the win is actually
  noticeable (dev friction or real performance) AND cheap in complexity.
  Algorithmic wins: go for it. Optimizations that add deps or tangle the
  structure for marginal gains: skip.
- **Write idiomatic Rust.** That is the priority, though not a blocker.

## Design system

Strictly consistent: one palette, one spacing scale, one radius/motion language.
All colors and sizes live in `src/theme.rs` and nowhere else.

Current style language: **flat minimal** — square corners only, no shadows, no
gradients, hairline (`border_*_1`) separators at most. The user plans to make it
fancier later, but every change must still flow through `theme.rs` tokens.

Palette = the "Javid" colorscheme from the user's `~/.config/kitty/kitty.conf`:

| role        | hex       |
|-------------|-----------|
| background  | `#0D120F` |
| foreground  | `#C8DCC7` |
| border      | `#354735` |
| green       | `#9FE37F` |
| yellow      | `#C4D87C` |
| sea green   | `#6FB88A` |
| purple      | `#B88AD9` |
| mint        | `#7ED6A1` |
| bright purple | `#D0A5EF` |
| bright green | `#A8E889` |
| light green | `#9BCF78` |
| light purple | `#C49BE3` |
| muted       | `#59665D` |
| brightest   | `#E0E9DC` |

Only these colors may appear in the UI; use the base (normal) variants first.

## Architecture

```
src/
  main.rs     entry point: startup tuning -> load config -> open window
  app.rs      root view: brand, nav (from config.sections); the `settings`
              section renders the config/data export-import panel
  config.rs   app STRUCTURE config (sections), TOML, validated, export/import
  data.rs     personal DATA store: private dir, directory-tree export/import
  paths.rs    XDG dirs: ~/.config/prog-rs (config) vs ~/.local/share/prog-rs (data)
  assets.rs   compile-time embedded assets (logo) via gpui AssetSource
  theme.rs    ALL design tokens: colors (Javid), geometry, spacing, type, weight
  startup.rs  pre-GPU env tuning (skip NVIDIA ICD when it drives no display)
assets/       source icon (pure-path SVG) + KDE .desktop template
scripts/      install-desktop-entry.sh (KRunner launcher installer)
```

- **Config vs data is the core boundary.** Config = structure (what sections
  exist), shareable. Data = user content, private, only moved by explicit
  user action. Both live under the app name in their XDG homes; config is
  created with defaults on first run (`~/.config/prog-rs/config.toml`).
- Menu sections render from `config.sections` — adding a section later is a
  config edit, not a code change (content views will attach per section).
  A section named `settings` (`app::SETTINGS_SECTION`) gets the settings
  panel instead of the placeholder.
- Settings panel: config export/import are wired through gpui's portal file
  pickers (`cx.prompt_for_new_path` / `cx.prompt_for_paths` — Zed's own Linux
  path, verified against the running xdg-desktop-portal). `data::export/import`
  exist and are tested but NOT wired to the UI yet — the data buttons show a
  "not implemented yet" notice.
- Window: maximized (restore size 960x600), Escape quits, last window closed
  quits. Font: 0xProto Nerd Font via fontconfig.

## Notes

- **App identity is a stub.** Names/descriptions like the KRunner
  `GenericName`, the README blurb, and the desktop `Comment=`, plus the
  placeholder "empty" content panes, are stand-ins. When the application
  matures and gains its real purpose, update these to match it
  (assets/prog-rs.desktop, README.md, `src/app.rs`).

- `assets/` holds the KRunner launcher template and the icon (hand-built SVG
  paths, no fonts — renders identically in any renderer).
  `scripts/install-desktop-entry.sh` installs both into the user's KDE env and
  rewrites `Exec=` to the absolute binary path (the GUI session's PATH is not
  guaranteed to include `~/.cargo/bin`).

- `src/startup.rs` skips the NVIDIA Vulkan driver at process start when the
  kernel reports no display connected to any NVIDIA GPU. This is what makes
  `prog-rs` launch in ~100ms instead of ~2s on this hybrid laptop (probing the
  sleeping NVIDIA driver dominates cold-start time). Remove the module if it
  ever misbehaves; the app works without it, just slower.

- `gpui` and `gpui_platform` come via **git over SSH** from
  `github.com:zed-industries/zed` (pinned by `Cargo.lock`). Do not mix with the
  crates.io `gpui` release — `gpui_platform` is not published there and the
  crates.io `gpui` has drifted.
- `.cargo/config.toml` sets `git-fetch-with-cli` so cargo uses the system git
  (the user's SSH setup is not visible to libgit2).
- Linux: requires wayland and/or x11 dev libraries; rendering uses wgpu/Vulkan.
