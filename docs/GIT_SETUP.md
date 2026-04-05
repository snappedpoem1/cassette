# Git Setup And Verification

Last updated: 2026-04-05

## Canonical Remote

This repository is expected to use this remote URL for both fetch and push:

- https://github.com/snappedpoem1/cassette.git

Verify:

```powershell
git remote -v
```

Expected shape:

- `origin ... (fetch)`
- `origin ... (push)`

## Branch Tracking

Primary branch is `main`, tracking `origin/main`.

Verify:

```powershell
git branch -vv
```

Expected shape for active branch:

- `main [origin/main]`

## Working Tree Health

Before commits or releases, ensure the working tree is clean unless you intentionally have in-progress work.

Verify:

```powershell
git status -sb
```

Healthy release-prep shape:

- `## main...origin/main`
- no modified/untracked file lines

## Commit Identity

Configure commit identity once per machine:

```powershell
git config --global user.name "Your Name"
git config --global user.email "you@example.com"
```

Verify:

```powershell
git config --get user.name
git config --get user.email
```

## Recommended Pull/Push Flow

```powershell
git checkout main
git pull --ff-only origin main
# make changes
git add -A
git commit -m "<summary>"
git push origin main
```

## Optional Safety Settings

Use fast-forward-only pulls to avoid accidental merge commits:

```powershell
git config --global pull.ff only
```

Use rebase-by-default only if that matches your team workflow:

```powershell
git config --global pull.rebase false
```

## Current Verified Snapshot (This Machine)

Verified on 2026-04-05:

- Active branch: `main`
- Upstream: `origin/main`
- Remote URL: `https://github.com/snappedpoem1/cassette.git`
- Working tree state after latest push: clean
- Commit identity configured locally
