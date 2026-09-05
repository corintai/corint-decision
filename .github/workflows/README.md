# GitHub Actions Workflows

This directory contains CI/CD workflows for the CORINT Decision Engine project.

## Workflow Overview

### 1. CI Workflow (`ci.yml`)

**Trigger Conditions**:
- Push to `main`
- Pull requests targeting `main` or `develop` branches

**Execution Details**:

#### CDL Core Job
- Runs real-engine conformance, strict CLI/toolchain/generator checks and isolated server tests
- Checks fixture mappings on three strict reference pages and scope/claim classification on compatibility pages
- Runs fixed-model delivery through real CLI and loopback HTTP processes
- See [Core process e2e](../../tests/CORE_E2E.md) for prerequisites and evidence boundaries

#### Test Job
- ✅ Code formatting check (`cargo fmt`)
- ✅ Clippy static analysis (`cargo clippy`)
- ✅ Run all unit tests (`cargo test`)
- ✅ Run documentation tests (`cargo test --doc`)

#### Build Job
- ✅ Build release version on Linux and macOS
- ✅ Build all examples
- ✅ Check `corint-decision-server` binary size
- Installs `protoc` explicitly on both build platforms

#### Coverage Job
- ✅ Generate code coverage using `cargo-tarpaulin`
- ✅ Upload coverage reports to Codecov

#### Security Audit Job
- ✅ Run `cargo audit` to check for dependency vulnerabilities

**Features**:
- 📦 Cargo cache for faster builds (registry, git, build)
- 🚀 Parallel job execution
- 🔧 Multi-platform build support (Ubuntu, macOS)

---

### 2. Release Workflow (`release.yml`)

The release workflow below still contains legacy package identifiers and needs a
separate release migration before use. Passing the Core/CI checks does not validate
this publishing workflow. These descriptions document configuration, not a completed
remote CI or release run.

**Trigger Conditions**:
- Push tags matching `v*.*.*` (e.g., `v1.0.0`)

**Execution Details**:

#### Create Release Job
- 📝 Create GitHub Release

#### Build Release Job
- 🔨 Build multi-platform release binaries:
  - Linux x86_64
  - macOS x86_64 (Intel)
  - macOS aarch64 (Apple Silicon)
- 📦 Package as `.tar.gz` archives
- ⬆️ Upload to GitHub Release

#### Publish Crate Job
- 📤 Publish all crates to crates.io in dependency order:
  1. `corint-core`
  2. `corint-parser`
  3. `corint-compiler`
  4. `corint-runtime`
  5. `corint-sdk`
  6. `corint-server`

---

## Usage Guide

### Local Development Validation

Before committing code, it's recommended to run the same checks locally:

```bash
# 1. Check code formatting
cargo fmt --all -- --check

# 2. Run Clippy
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings

# 3. Build the server required by process tests, then run tests
# Requires protoc; stop if building the server fails.
CORINT_E2E_SERVER=$(bash tests/scripts/run_core_e2e_tests.sh --build-only) && \
  export CORINT_E2E_SERVER && \
  cargo test --all-features --workspace --locked

# 4. Build release version
cargo build --release --all-features --workspace
```

### Viewing CI Results

After committing code, you can check CI status in the following locations:

1. **Repository page**: CI status badge at the top
2. **Pull Request page**: All checks status at the bottom
3. **Actions tab**: Detailed execution logs

### Publishing a New Release

1. **Update version numbers**:
   ```bash
   # Edit version in all Cargo.toml files
   vim crates/*/Cargo.toml
   ```

2. **Commit and tag**:
   ```bash
   git add .
   git commit -m "Bump version to 1.0.0"
   git tag v1.0.0
   git push origin main
   git push origin v1.0.0
   ```

3. **Automatic trigger**:
   - After pushing the tag, the `release.yml` workflow will automatically trigger
   - Once the build completes, binaries can be downloaded from the GitHub Releases page

---

## Configuring Secrets

Some workflows require GitHub Secrets to be configured:

### Codecov Token (Optional)

1. Visit [https://codecov.io](https://codecov.io)
2. Link your GitHub repository
3. Copy the token
4. Add the secret to your GitHub repository: `CODECOV_TOKEN`

### Cargo Token (For publishing to crates.io)

1. Visit [https://crates.io/settings/tokens](https://crates.io/settings/tokens)
2. Generate a new API token
3. Add the secret to your GitHub repository: `CARGO_TOKEN`

**Steps to add a Secret**:
1. Go to your GitHub repository page
2. Settings → Secrets and variables → Actions
3. Click "New repository secret"
4. Enter the name and value

---

## Badges

You can add the following badges to your README.md:

```markdown
![CI](https://github.com/YOUR_USERNAME/corint-decision/workflows/CI/badge.svg)
[![codecov](https://codecov.io/gh/YOUR_USERNAME/corint-decision/branch/main/graph/badge.svg)](https://codecov.io/gh/YOUR_USERNAME/corint-decision)
```

---

## Troubleshooting

### Common CI Failure Causes

1. **Formatting check failed**:
   ```bash
   cargo fmt
   ```

2. **Clippy warnings**:
   ```bash
   cargo clippy --all-targets --all-features --fix
   ```

3. **Test failures**:
   ```bash
   # Run tests locally with detailed output
   RUST_LOG=debug cargo test -- --nocapture
   ```

4. **Dependency cache issues**:
   - Manually re-run the workflow from the GitHub Actions page
   - If failures persist, disable caching in the workflow

---

## Performance Optimization

### Speeding Up CI

1. **Use cargo cache** (already enabled)
2. **Run jobs in parallel** (already configured)
3. **Run coverage only when needed**:
   - Coverage job can be set to run only on the main branch

4. **Reduce test matrix**:
   - If build times are too long, reduce the OS matrix

### Reducing GitHub Actions Minutes Consumption

```yaml
# Run coverage only on main branch
coverage:
  if: github.ref == 'refs/heads/main'
  # ...
```

---

## Advanced Configuration

### Add Performance Benchmarks

```yaml
benchmark:
  name: Benchmark
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - uses: dtolnay/rust-toolchain@stable
    - run: cargo bench --all-features
```

### Add Docker Image Build

```yaml
docker:
  name: Build Docker Image
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - name: Build image
      run: docker build -t corint-server:${{ github.sha }} .
    - name: Push to registry
      run: |
        echo ${{ secrets.DOCKER_PASSWORD }} | docker login -u ${{ secrets.DOCKER_USERNAME }} --password-stdin
        docker push corint-server:${{ github.sha }}
```

---

## Reference Documentation

- [GitHub Actions Documentation](https://docs.github.com/en/actions)
- [Cargo Documentation](https://doc.rust-lang.org/cargo/)
- [rust-cache Action](https://github.com/Swatinem/rust-cache)
- [cargo-tarpaulin](https://github.com/xd009642/tarpaulin)
