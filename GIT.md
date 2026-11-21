# Git policy (solo project)

I am the only developer. There are no branches, no PRs, no review. Everything
happens on `main`. Your job is not to protect me from myself with process — it's
to keep history clean, reversible, and honest, and to never destroy work.

## Every session, before touching anything

1. `git status` — understand the working tree. Uncommitted changes may be mine
   and in progress. Never revert, checkout, stash, or clean files you did not
   create in this session. If something unexpected is dirty, ask before touching it.
2. `git fetch` and compare `main` with `origin/main`. If behind, `git pull --ff-only`.
   Never build on a stale base. If it cannot fast-forward, stop and tell me.

## Committing

- Commit coherent units: one logical change, complete and self-contained.
  Do not bundle unrelated edits into one commit.
- Stage specific files. Never `git add -A` or `git add .` blindly.
- Before committing, run `git diff --staged` and check for secrets, `.env`,
  credentials, generated output, and accidental unrelated changes.
- Message: imperative subject, at most ~72 characters, stating what changed.
  A body only when it adds real information — the *why* or a non-obvious
  constraint. Do not restate the diff or pad it.
- No "WIP", "fix", "updates", "misc" messages. If the change needs a vague
  message, it needs to be split.

## Pushing

- Push when work is in a coherent, working state. Do not accumulate commits
  locally for days; my history is my only safety net.
- Never push if tests/builds are knowingly broken, unless I ask for it.

## Never do without explicit permission

- `push --force` / `--force-with-lease` on `main`.
- `rebase`, `reset --hard`, `commit --amend` on commits that are already pushed.
- Deleting tags, remote refs, or the remote's history.
- `git clean -fd`, discarding stashes, or dropping uncommitted work.
- Rewriting `.git/config`, remotes, or any git internals.
- Anything whose explanation includes the phrase "to fix the git state".

Amending is fine for commits that are still local and unpushed.

## When things get weird

Prefer the boring, reversible option. If you find yourself about to do something
you'd have to justify twice, stop and describe the situation and your options in
two or three sentences and ask. A tangled history is far more expensive than an
interruption.

But do not escalate routine work: committing a finished change, pulling before
work, or writing a message are not questions — just do them.

## Definition of a good end state

When you finish, I should be able to run `git log --oneline` and read a clear
narrative of what happened, `git status` should be clean, and `origin/main`
should be at or ahead of where we started with no history rewritten.
