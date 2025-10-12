# Codecov Setup Guide

## Quick Setup (5 minutes)

### 1. Register on Codecov

1. Go to https://codecov.io/
2. Sign in with your GitHub account
3. **Important**: Switch to organization `ytget` (not `romanitalian`)
4. Click "Add new repository" 
5. Select `ytget/ryt` from the list

**If you don't see `ytget` organization:**
- Click "Install Codecov" button
- Install Codecov app for organization `ytget` on GitHub
- Grant necessary permissions

### 2. Get Codecov Token

After adding the repository:
1. Go to repository settings on Codecov (https://app.codecov.io/gh/ytget/ryt/settings)
2. Copy the "Repository Upload Token" (starts with something like `a1b2c3d4-...`)

**Direct link to repository**: https://app.codecov.io/gh/ytget/ryt

### 3. Add Token to GitHub

1. Go to https://github.com/ytget/ryt/settings/secrets/actions
2. Click "New repository secret"
3. Name: `CODECOV_TOKEN`
4. Value: paste your token from step 2
5. Click "Add secret"

### 4. Trigger CI

Push any commit or create a Pull Request to trigger the CI workflow:

```bash
git add .
git commit -m "feat(ci): add codecov integration"
git push
```

### 5. View Coverage Report

After CI completes:
- Visit: https://codecov.io/gh/ytget/ryt
- Badge will automatically update in README.md

## What You Get

### Coverage Badge
Shows current test coverage percentage:
[![codecov](https://codecov.io/gh/ytget/ryt/branch/main/graph/badge.svg)](https://codecov.io/gh/ytget/ryt)

### Detailed Reports
- Line-by-line coverage visualization
- Coverage trends over time
- PR coverage diff (shows coverage changes in PRs)
- File tree with coverage percentages

### PR Comments
Codecov bot will comment on PRs with:
- Coverage percentage change
- New uncovered lines
- Files with decreased coverage

## Configuration

The `codecov.yml` file configures:

- **Project coverage target**: 70% minimum
- **Patch coverage**: New code should have 70% coverage
- **Threshold**: Allow 1% decrease in project coverage
- **Ignored files**: Tests, examples, build artifacts

## Targets

```yaml
coverage:
  range: "70...100"  # Green at 70%+, red below
  
  status:
    project:
      target: 70%      # Entire project target
      threshold: 1%    # Allow 1% decrease
    
    patch:
      target: 70%      # New code target
      threshold: 5%    # Allow 5% variance
```

## Local Coverage Check

Run coverage locally before pushing:

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --out Html --output-dir coverage

# Open report
open coverage/index.html
```

## Troubleshooting

### Repository not found in Codecov
**Problem**: Codecov shows "No results found" for ytget/ryt
**Solution**: 
1. Switch to organization `ytget` in Codecov (not `romanitalian`)
2. If `ytget` org not visible - install Codecov app for it on GitHub
3. Use direct link: https://app.codecov.io/gh/ytget/ryt

### Badge shows "unknown"
- Wait 2-3 minutes after first CI run
- Check if CI workflow completed successfully
- Verify CODECOV_TOKEN is set correctly

### Coverage not uploading
- Check GitHub Actions logs for coverage job
- Verify codecov-action has correct token
- Ensure tarpaulin generated cobertura.xml

### Low coverage warnings
This is normal for new projects. Improve by:
1. Adding unit tests for core modules
2. Adding integration tests
3. Testing error paths and edge cases

## Advanced: Coverage Goals

Current targets (adjust in codecov.yml):
- **70%**: Minimum acceptable
- **80%**: Good coverage
- **90%+**: Excellent coverage

For this project, aim for:
- Core logic: 85%+
- Utils: 80%+
- CLI: 70%+
- Overall: 70%+

## Resources

- [Codecov Documentation](https://docs.codecov.io/)
- [cargo-tarpaulin](https://github.com/xd009642/tarpaulin)
- [Rust Testing Guide](https://doc.rust-lang.org/book/ch11-00-testing.html)

