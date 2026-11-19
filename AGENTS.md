# AGENTS.md

## Project

prog-rs — a [GPUI](https://gpui.rs) (Zed's GPU-accelerated UI framework) app.
Currently shows an animated gradient in a 100x100 px rounded box; installs on
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
- **Every commit message must explain well what is happening and why** —
  a precise subject plus a body describing the change and its reasoning,
  not just a one-line label.

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

## Notes

- **App identity is a stub.** Names/descriptions like the KRunner
  `GenericName=Animated gradient`, the README blurb, and the desktop
  `Comment=` are placeholders. When the application matures and gains its
  real purpose, update these to match it (assets/prog-rs.desktop, README.md).

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
