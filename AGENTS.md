# AGENTS.md

## Project

prog-rs — a [GPUI](https://gpui.rs) (Zed's GPU-accelerated UI framework) app.
It will become a personal life-metrics dashboard: turning the user's habits
and goals into graphs and stats — how well he's saving money, staying off
porn, losing weight, keeping up skincare, etc. Fully flexible/unbounded
because the user builds it himself; structure still being figured out.
`main` shows the big picture: current trajectory vs. the baseline "him who
didn't focus on any of this" (e.g. "you wouldn't have saved X CZK, you'd
weigh 10kg more, ..."). Sections (finances, appeal, ...) map to life areas.
The shell, config/data stores, finances, study attendance and the appeal
relapse clock are implemented; other sections are still placeholders.

## Commands

- Build: `cargo build --release`
- Run: `cargo run`
- Test: `cargo test`
- Install on PATH: `cargo install --path . --force` (installs to
  `~/.cargo/bin/prog-rs`; `--force` is needed to replace the same version)
- Install KDE launcher (KRunner entry + icon): `scripts/install-desktop-entry.sh`

## Workflow rules

- **Git usage follows `GIT.md`.** Status/fetch before work, stage specific
  files, commit coherent units, never rewrite history without permission.
- **After every change, install a fresh build for KRunner.** Run
  `cargo install --path . --force` before handing work back. The KDE
  launcher's `Exec=` points at `~/.cargo/bin/prog-rs`, so without this
  KRunner keeps launching the previous build. A change is not done until
  KRunner launches it.
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
  app.rs      root view: brand, nav (from config.sections), per-tab settings,
              `main` finance headline, and the single scrollable content area
              with custom scrollbar
  config.rs   app STRUCTURE config: sections + shared currency rates, TOML,
              validated, export/import
  currency.rs Currency/Rates types plus shared decimal parsing/formatting
  data.rs     personal DATA store: private dir, directory-tree export/import
  modal.rs    app-wide confirmation dialog: full-window dim + centered panel
  appeal/
    mod.rs    appeal view: second-ticking relapse-free clock and reset flow
    model.rs  v1 appeal.toml: last_relapse timestamp; calendar month/day clock
  study/
    mod.rs    study view: Mon-Fri month calendar, attendance cycling, stats
    schedule.rs fixed 2026 winter timetable; generates sessions Nov 1-Jan 10
    model.rs  v1 study.toml: attendance map keyed by session
  finances/
    mod.rs    finance view: category/window toolbar, slideshow, chart,
              window + correction panels, entry form, ledger
    model.rs  v7 finances.toml: payout windows (category balances or combined),
              transactions (expense/income, optional foreign currency),
              manual balance corrections; all amounts in minor units (0.01 CZK)
    stats.rs  per-window/category math: available money, average plan,
              break-even/day, points, slideshow statements
    graph.rs  custom-painted chart (actual + average plan) with hover readout
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
- Menu sections render from `config.sections`; adding a section is a config
  edit, not a code change. A section named `settings`
  (`app::SETTINGS_SECTION`) renders one settings block per configured section
  (finances supplies currency conversion rates; others show an empty note).
  `finances`, `study` and `appeal` are the real content views so far.
- Config vs data stays strict: currency rates live in `config.toml`, while
  payout windows, transactions, balance corrections, study attendance and the
  appeal `last_relapse` live in private files under the data dir. All have validated
  formats and are covered by directory export/import.
- Finances: a window is a payout period with per-category balances
  (nutrition/allowance) or a single `combined` balance. Transactions are
  expense/income, optionally entered in CZK/EUR/AZN and converted to CZK at
  entry time using the configured rates. All finance amounts are stored in
  hundredths of a crown and displayed with 0.01 precision; older whole-crown
  data is scaled on load. Manual corrections store a CZK delta
  at a timestamp so later points shift automatically. Chart draws the stepped
  actual balance line and the straight average plan from the window's first
  income; hovering shows minute-precision values for both. A single
  break-even/day readout (`remaining / days left`) is the primary number. The
  slideshow has exactly three shared stats per view: signed gap from the
  average plan, last-7-days daily average, and percent of that view's money
  left. `main` shows the same slideshow with combined (default), nutrition and
  allowance options for the current window.
- Study is a fixed winter-semester timetable in `study/schedule.rs` (1 Nov
  2026 - 10 Jan 2027; summer is intentionally absent). Attendance is the only
  user data, stored in `study.toml` with `attended`/`missed` values. The
  calendar is Monday-Friday only, shows short course ids as full-width event
  cards (time + type badge + short id, type-colored accent, today tinted), and
  clicking a card cycles unmarked -> attended -> missed -> unmarked; future
  sessions are not clickable and are dimmed. Monthly
  stats count started classes, with unmarked treated as not attended, for all
  classes, lectures, and labs+seminars; future sessions are not markable.
  The occasional "odd weeks" lab uses ISO week parity until a real academic
  week anchor is available. `main` shows the same three-number stats row for
  the current month.
- Appeal stores only `last_relapse` in `appeal.toml` and derives a
  `MM:DD:HH:MM:SS` clock (calendar months, then days/hours/minutes/seconds) in
  the UI. The appeal tab is a centered square panel with a `RELAPSE-FREE`
  header, the last-relapse timestamp, five labeled segments
  (months/days/hours/minutes/seconds) and a `RESET CLOCK` button; resetting is
  confirmed through `modal.rs`, which dims the whole app and where Escape
  cancels instead of quitting. `main` shows the same clock without the reset
  control.
- `main` groups every section summary under an explicit section label (for now
  FINANCES, STUDY and APPEAL) via `AppRoot::main_block`, separated by
  hairlines. New
  sections with a main summary must be added there so ownership stays obvious.
- Every tab's content lives in one shared vertical scroller with a custom
  right-edge scrollbar that only paints when content overflows; it widens on
  hover and is draggable. The sidebar list scrolls independently.
- Settings panel: config export/import are wired through gpui's portal file
  pickers (`cx.prompt_for_new_path` / `cx.prompt_for_paths` — Zed's own Linux
  path, verified against the running xdg-desktop-portal). `data::export/import`
  exist and are tested but NOT wired to the UI yet — the data buttons show a
  "not implemented yet" notice.
- Window: maximized (restore size 960x600), Escape quits, last window closed
  quits. Font: 0xProto Nerd Font via fontconfig.

## Notes

- **App identity: life-metrics dashboard.** The finances model is settled
  enough to work with (windows, categories/combined, corrections, rates);
  other life areas are still undecided, so keep their UI generic.

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
