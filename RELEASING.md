# Releasing turbocow

## CI (GitHub Actions)

Workflows live in `.github/workflows/`:

- `ci.yml` — on every push / PR, weekly, and on demand: fmt + check + tests on
  the MSRV (1.85.0, library only) / beta / stable matrix, the feature matrix,
  docs.rs-equivalent docs, allocator-feature mutual exclusivity,
  `nightly-allocator-api`, clippy, loom, Miri (x86_64, aarch64, sparc64 BE,
  i686), and `cargo package`.
- `coverage.yml` — `cargo llvm-cov` per feature set; `lcov.info` is uploaded as
  a workflow artifact. To publish to Codecov, add a `CODECOV_TOKEN` repository
  secret and uncomment the upload step.
- `fuzz.yml` — weekly (and manual) 2-minute smoke runs of every `fuzz/` target.

All jobs run on GitHub-hosted `ubuntu-latest` runners; Miri caches its
per-target sysroot (`~/.cache/miri`) and `target/miri` with `actions/cache`.
Third-party actions are pinned to full commit SHAs — bump them together.

Two repo-local quirks the workflows already neutralize:

- `.cargo/config.toml` pins clang + the `wild` linker for fast local builds;
  CI clears the `wild` link-arg and uses plain clang.
- `rust-toolchain.toml` pins a specific nightly; CI sets `RUSTUP_TOOLCHAIN`
  per job so the MSRV / beta / stable matrix is actually exercised.

## Testing

Beyond the unit/integration tests:

- **Differential proptests** (`tests/proptest_*.rs`) apply an `arbitrary`-decoded
  op sequence to each turbocow collection and to a `std` oracle (and, for the
  ecow-derived types, to `ecow` itself), asserting equivalence after every op.
  They run in plain `cargo test`. Tune with `PROPTEST_CASES=<n>`.
- **Fuzzing** (`fuzz/`) reuses the *same* model via `cargo-fuzz`:
  `cargo +nightly fuzz run <smallvec|ecovec|ecostring|ecobytes|map|set>`. A crash drops a
  reproducer under `fuzz/artifacts/`; the input replays in the proptest suite.
- **Miri replay** catches UB the above can't see at the value level. `cargo miri
  test --tests` runs the proptests too — cap them and disable isolation (proptest
  persists regression files to disk), e.g.
  `PROPTEST_CASES=16 MIRIFLAGS="-Zmiri-disable-isolation -Zmiri-strict-provenance" cargo miri test --tests`.
  CI runs this across LE / BE / 32-bit.

## Publishing a version

1. Bump `version` in `Cargo.toml` and update `CHANGELOG.md`.
2. Confirm green CI on `master`.
3. `cargo publish --dry-run` (the `package` CI job runs `cargo package`).
4. Tag the release: `git tag v<version> && git push origin v<version>`.
5. `cargo publish`.

The MSRV (1.85) should be verified against a real toolchain before publishing,
e.g. `RUSTUP_TOOLCHAIN=1.85.0 cargo +1.85.0 check` — the pinned nightly in
`rust-toolchain.toml` otherwise masks MSRV regressions locally.
