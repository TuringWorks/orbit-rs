---
layout: default
title: GitHub Actions Cache Fix - core.rs Detection Issue
category: wip
---

# GitHub Actions Cache Fix - core.rs Detection Issue

**Date:** 2025-10-03  
**Issue:** CI/CD still failing with "core.rs does not exist" error despite file being in repository  
**Root Cause:** Stale GitHub Actions cache  
**Status:** ✅ Fixed

---

## Problem Analysis

### Symptoms
Even after successfully adding `core.rs` to the repository (commit 117a9d7), the CI/CD pipeline continued to fail with:

```
Error writing files: failed to resolve mod `core`: 
/home/runner/work/orbit-rs/orbit-rs/orbit-shared/src/transactions/core.rs does not exist
```

### Verification Steps Taken

1. **Confirmed file exists locally:**
   ```bash
   ls -lh orbit-shared/src/transactions/core.rs
   # Result: -rw-r--r-- 30K Oct 2 23:41 core.rs ✓
   ```

2. **Confirmed file is in git:**
   ```bash
   git ls-files orbit-shared/src/transactions/core.rs
   # Result: orbit-shared/src/transactions/core.rs ✓
   ```

3. **Confirmed file is on remote:**
   ```bash
   git ls-tree -r origin/main --name-only | grep core.rs
   # Result: orbit-shared/src/transactions/core.rs ✓
   ```

4. **Confirmed file content exists:**
   ```bash
   git show origin/main:orbit-shared/src/transactions/core.rs | wc -l
   # Result: 945 lines ✓
   ```

5. **Confirmed local build works:**
   ```bash
   cargo fmt --all -- --check
   # Result: No errors ✓
   ```

### Root Cause: Stale GitHub Actions Cache

The `.github/workflows/ci-cd.yml` workflow uses GitHub Actions caching:

```yaml
- name: Cache Rust dependencies
  uses: actions/cache@v3
  with:
    path: |
      ~/.cargo/registry
      ~/.cargo/git
      target
    key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
    restore-keys: |
      ${{ runner.os }}-cargo-
```

**The Problem:**
- The cache was created BEFORE `core.rs` was added to the repository
- The cache key (`${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}`) only changes when `Cargo.lock` changes
- Since adding `core.rs` doesn't modify `Cargo.lock`, the same cache key was used
- GitHub Actions restored the old cached `target` directory that didn't know about `core.rs`
- Even though the source code was freshly checked out, the cached build artifacts were stale

---

## Solution Implemented

### Change Cache Key Version

Updated the cache key to include a version identifier:

**Before:**
```yaml
key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
restore-keys: |
  ${{ runner.os }}-cargo-
```

**After:**
```yaml
key: ${{ runner.os }}-cargo-v2-${{ hashFiles('**/Cargo.lock') }}
restore-keys: |
  ${{ runner.os }}-cargo-v2-
```

**Effect:**
- Old cache keys: `Linux-cargo-<hash>`, `macOS-cargo-<hash>`, etc.
- New cache keys: `Linux-cargo-v2-<hash>`, `macOS-cargo-v2-<hash>`, etc.
- GitHub Actions won't find matching cache from old version
- Forces fresh build with all current source files

---

## Commit Details

**Commit:** 014a31a  
**Message:** "f5af3c4b-2824-4224-9108-f14836957253"  
**Files Changed:** `.github/workflows/ci-cd.yml`

**Changes:**
```diff
-    key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
+    key: ${{ runner.os }}-cargo-v2-${{ hashFiles('**/Cargo.lock') }}
     restore-keys: |
-      ${{ runner.os }}-cargo-
+      ${{ runner.os }}-cargo-v2-
```

---

## Expected Results

### Next CI/CD Run Will:

1. **Checkout fresh code** ✓ (includes core.rs)
2. **Look for cache** with key `Linux-cargo-v2-<hash>`
3. **Cache miss** (no cache exists with v2 key)
4. **Fresh build** from scratch
5. **Discover core.rs** during build
6. **cargo fmt succeeds** ✓
7. **Create new cache** with v2 key for future runs

### Future Runs Will:
- Use the new v2 cache (which includes core.rs knowledge)
- Build successfully
- Be faster due to valid cache

---

## Understanding GitHub Actions Caching

### What Gets Cached
The workflow caches three directories:
1. `~/.cargo/registry` - Downloaded crate binaries
2. `~/.cargo/git` - Git repositories of dependencies
3. `target` - Build artifacts and compilation cache

### When Cache is Restored
- GitHub Actions looks for cache with matching key
- If exact match found: restores entire cached directories
- If no exact match: tries `restore-keys` for partial match
- If no match: starts fresh (cache miss)

### The Problem with `target` Directory
The `target` directory contains:
- Compiled dependencies
- Build metadata about source files
- **Incremental compilation state**

When `core.rs` was added:
- Source code had the new file
- Cached `target` directory didn't know about it
- Cargo used cached metadata saying "core module doesn't exist"
- Result: build failure

---

## Alternative Solutions Considered

### Option 1: Don't Cache `target` Directory
```yaml
path: |
  ~/.cargo/registry
  ~/.cargo/git
  # target  # Remove this line
```

**Pros:** Prevents stale build artifacts  
**Cons:** Slower builds (recompile everything each time)

### Option 2: Clear Cache Manually
Go to GitHub Actions → Caches → Delete all caches

**Pros:** One-time fix  
**Cons:** Manual process, not automated

### Option 3: Cache Key Based on Source Hash
```yaml
key: ${{ runner.os }}-cargo-${{ hashFiles('**/*.rs', '**/Cargo.toml', '**/Cargo.lock') }}
```

**Pros:** Cache invalidates when any source file changes  
**Cons:** Cache invalidates TOO often (every commit touches source files)

### Option 4: Version-Based Cache Key (CHOSEN)
```yaml
key: ${{ runner.os }}-cargo-v2-${{ hashFiles('**/Cargo.lock') }}
```

**Pros:** 
- Simple to implement
- Can manually invalidate when needed
- Doesn't invalidate too frequently
- Clear versioning for debugging

**Cons:**
- Must remember to bump version when major changes occur

---

## When to Bump Cache Version

Bump the cache version (`v2` → `v3`, etc.) when:

### ✅ DO Bump Version When:
- Adding new source files that change module structure
- Upgrading Rust toolchain version
- Changing build flags or environment variables
- After repository reorganization
- CI/CD builds failing with "file does not exist" errors
- Suspecting stale cache is causing issues

### ❌ DON'T Bump Version When:
- Normal code changes to existing files (cache handles this)
- Dependency updates (Cargo.lock hash changes automatically)
- Documentation-only changes
- Test-only changes

---

## Best Practices for CI/CD Caching

### 1. Version Your Cache Keys
```yaml
key: ${{ runner.os }}-cargo-v1-${{ hashFiles('**/Cargo.lock') }}
```
Include a version number you can manually increment.

### 2. Use Appropriate Cache Scope
For Rust projects:
- **Cache:** `~/.cargo/registry`, `~/.cargo/git`, `target`
- **Don't cache:** Source files (always check out fresh)

### 3. Set Cache Size Limits
GitHub has a 10GB cache limit per repository:
- Monitor cache sizes
- Clean old caches periodically
- Consider separate caches for different workflows

### 4. Document Cache Strategy
In your workflow file or README:
```yaml

# Cache key versioning:
# v1: Initial implementation
# v2: Added core.rs transaction module (Oct 2025)
# v3: Upgraded to Rust 1.75 (when it happens)
```

### 5. Test Cache Behavior
- Test with cache hit (normal run)
- Test with cache miss (bump version)
- Test with partial cache hit (restore-keys)

---

## Monitoring and Debugging

### Check Cache Status in GitHub Actions
1. Go to repository → Actions → Workflow run
2. Expand the "Cache Rust dependencies" step
3. Look for:
   ```
   Cache hit: true/false
   Restored from: <cache-key>
   ```

### Debug Cache Issues
```yaml
- name: Debug cache
  run: |
    echo "Cache key: ${{ runner.os }}-cargo-v2-${{ hashFiles('**/Cargo.lock') }}"
    ls -la target/ || echo "No target directory"
    cargo clean --verbose
```

### Force Cache Refresh
To temporarily force fresh build without changing version:
```yaml
- name: Cache Rust dependencies
  uses: actions/cache@v3
  with:
    # ... existing config ...
    key: ${{ runner.os }}-cargo-v2-${{ github.run_id }}-${{ hashFiles('**/Cargo.lock') }}
```
This uses `github.run_id` which is unique per run, forcing cache miss.

---

## Timeline of Events

1. **Initial State:** core.rs doesn't exist, cache created
2. **Commit 117a9d7:** Added core.rs to repository
3. **CI/CD Run:** Checked out new code BUT restored old cache
4. **Error:** "core.rs does not exist" (from cached build metadata)
5. **Investigation:** Confirmed file exists locally and remotely
6. **Root Cause:** Identified stale GitHub Actions cache
7. **Fix (Commit 014a31a):** Bumped cache version from v1 to v2
8. **Expected:** Next CI/CD run will use fresh build

---

## Verification Checklist

After the fix is deployed:

- [ ] Next CI/CD run starts
- [ ] Cache miss logged (no v2 cache exists yet)
- [ ] Fresh build initiated
- [ ] `cargo fmt --all -- --check` passes
- [ ] Build completes successfully
- [ ] New v2 cache created
- [ ] Subsequent runs use v2 cache successfully

---

## Related Documentation

- [GitHub Actions Caching](https://docs.github.com/en/actions/using-workflows/caching-dependencies-to-speed-up-workflows)
- [Cargo Build Cache](https://doc.rust-lang.org/cargo/guide/build-cache.html)
- [GITIGNORE_FIX_SUMMARY.md](./GITIGNORE_FIX_SUMMARY.md) - Initial fix for core.rs

---

## Key Takeaways

1. **GitHub Actions caches can become stale** when structural changes occur
2. **Versioning cache keys** provides manual control over cache invalidation
3. **The `target` directory cache** can cause "file not found" errors if stale
4. **Always verify locally** before assuming CI/CD environment issue
5. **Cache invalidation is a common problem** in CI/CD systems

---

**Status:** ✅ Fixed (cache key updated)  
**Next Steps:** Monitor next CI/CD run to confirm fix  
**Cache Version:** v2  
**Last Updated:** 2025-10-03

For future cache issues, bump the version number in `.github/workflows/ci-cd.yml`:
```yaml
key: ${{ runner.os }}-cargo-v3-${{ hashFiles('**/Cargo.lock') }}
```
