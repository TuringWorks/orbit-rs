# GitFlow for LLMs: An Execution Playbook

## 1) Branch Roles (single source of truth)
- **`main`**: only production-ready code. Every commit on `main` is a released build and must be **tagged**.
- **`develop`**: integration branch for the next release. Always buildable/green.
- **`feature/*`**: short-lived branches off `develop` for new work. Merged into `develop`.
- **`release/*`**: harden a specific version; off `develop`, into `main` and back-merge into `develop`.
- **`hotfix/*`**: emergency production fix; off `main`, into `main` **and** `develop`.
- **`support/*`** (optional): long-term support lines for old majors; off a tagged release, into that support line and back-merged as policy dictates.

## 2) Naming Conventions (machine-parseable)
- `feature/<ticket|-|slug>` (e.g., `feature/ABC-123-user-onboarding`)
- `release/<semver>` (e.g., `release/1.4.0`)
- `hotfix/<semver>` (e.g., `hotfix/1.4.1`)
- `support/<major.minor>` (e.g., `support/1.4`)

**Commit messages**: Conventional Commits (`type(scope)!: summary`).  
**Tags**: `v<semver>` (e.g., `v1.4.0`).

## 3) Merge & Protection Rules
- `main`: protected; only PRs from `release/*` or `hotfix/*`. Require CI green, code review, and tag on merge.
- `develop`: protected; only PRs from `feature/*` or `release/*` (back-merge). Require CI.
- **Merge strategy**: “merge commit” (no fast-forward) to preserve branch history.  
  Optionally “squash” for feature branches—decide once and document.

## 4) Versioning Policy
- Use **SemVer**:
  - **major**: breaking change
  - **minor**: new features, no breaking change
  - **patch**: bug fixes only
- **Release branch** sets the target version; hotfix uses next patch on `main`.

---

## 5) Standard Operating Procedures

### 5.1 Feature Work
```bash
git checkout develop
git pull origin develop
git checkout -b feature/ABC-123-user-onboarding
# work, commit, push
git push -u origin feature/ABC-123-user-onboarding
# open PR → base: develop
```

### 5.2 Cutting a Release
```bash
export VERSION=1.4.0
git checkout develop && git pull
git checkout -b release/${VERSION}
# bump versions, update changelog
git commit -am "chore(release): prepare v${VERSION}"
git push -u origin release/${VERSION}
```

Finish release:
```bash
git checkout main
git merge --no-ff release/${VERSION}
git tag -a v${VERSION} -m "Release v${VERSION}"
git push origin v${VERSION}
git checkout develop
git merge --no-ff release/${VERSION}
```

### 5.3 Hotfix
```bash
export VERSION=1.4.1
git checkout main && git pull
git checkout -b hotfix/${VERSION}
# fix, commit
git push -u origin hotfix/${VERSION}
```

Finish hotfix:
```bash
git checkout main
git merge --no-ff hotfix/${VERSION}
git tag -a v${VERSION} -m "Hotfix v${VERSION}"
git push origin v${VERSION}
git checkout develop
git merge --no-ff hotfix/${VERSION}
```

### 5.4 Support Branches
- Create `support/<major.minor>` from the tag.
- Hotfixes merge back into that branch and get tagged.

---

## 6) Back-Merge Matrix

| Source → Target | develop | main | release/x.y.z | hotfix/x.y.z |
|---|---:|---:|---:|---:|
| feature/*       | ✅ | ❌ | ❌ | ❌ |
| release/*       | ✅ | ✅ | ✅ | ❌ |
| hotfix/*        | ✅ | ✅ | ✅ | ✅ |

---

## 7) CI/CD Hookpoints
- PR to `develop`: lint, unit tests, build.
- PR to `main`: above + e2e/integration, security scan.
- Tag: build & publish artifacts, container images, notify.

---

## 8) Conflict & Divergence
- Prefer **back-merge** (`git merge`) over cherry-pick.
- If a release branch is open and a hotfix ships → merge hotfix into main, develop, and release.

---

## 9) Minimal Command Cookbook

**Sync:**
```bash
git fetch --all --prune
git checkout develop && git pull
```

**Feature:**
```bash
git checkout -b feature/ABC-123-something develop
```

**Release:**
```bash
git checkout -b release/1.5.0 develop
```

**Hotfix:**
```bash
git checkout -b hotfix/1.5.1 main
```

---

## 10) Automation Policy (YAML)
```yaml
gitflow:
  protected_branches:
    - main
    - develop
  allowed_sources:
    main:
      - release/*
      - hotfix/*
    develop:
      - feature/*
      - release/*
      - hotfix/*
  branch_patterns:
    feature: ^feature\/[a-zA-Z0-9._-]+$
    release: ^release\/(\d+)\.(\d+)\.(\d+)$
    hotfix:  ^hotfix\/(\d+)\.(\d+)\.(\d+)$
  commit_message:
    conventional_commits: true
  merges:
    strategy: merge-commit
    require_ci_green: true
    require_reviews:
      main: 2
      develop: 1
  tagging:
    on_merge_to_main_from:
      - release/*
      - hotfix/*
    tag_format: v{version}
  versioning:
    scheme: semver
    release_branch_sets_version: true
  backmerge:
    hotfix_to:
      - develop
      - release/*
    release_to:
      - main
      - develop
```

---

## 11) Defaults (for LLMs)
- Production bug? → hotfix off `main`, merge to `main`, back-merge to `develop` (and `release/*`).
- New feature? → feature off `develop`, merge to `develop`.
- Preparing shipment? → release off `develop`, merge to `main`, tag, back-merge to `develop`.
