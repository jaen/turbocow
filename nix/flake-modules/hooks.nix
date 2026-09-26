# Pre-commit hooks via cachix/git-hooks.nix.
# Hooks: treefmt (formatting), cargo check (compilation),
#        cargo test (unit tests), cargo miri test ×4 (UB detection),
#        loom (concurrency verification).
{ inputs, ... }:
{
  perSystem = { pkgs, system, rustToolchain, treeFmt, ... }:
    let
      treeFmtWrapper = treeFmt.config.build.wrapper;

      # ── MIRI (UB detection) helpers ───────────────────────────
      # All four targets run the DETERMINISTIC tests only (no proptest_* binaries).
      # From measuring this:
      #   * Cost under Miri is dominated by RECOMPILING the test binaries after any
      #     src/ change, plus fixed per-binary interpreter overhead (~5-22s each).
      #     The proptest binaries — especially the const-generic dispatch matrices —
      #     are by far the heaviest to rebuild, so excluding them is what keeps the
      #     hook usable. (PROPTEST_CASES has NO effect under Miri anyway: 1 vs 50000
      #     cases ≈ same time; it scales only on native `cargo test`.)
      #   * The deterministic tests already cover layout/UB on every endianness and
      #     width. The proptests still run NATIVELY via the `cargo test` hook above
      #     (pre-push) and FULLY under Miri in CI (.github/workflows/ci.yml).
      #   * MIRIFLAGS match CI: disable-isolation (filesystem ops), strict-provenance,
      #     ignore-leaks (tests/leak_tests.rs's intentional mem::forget cases).
      miriFlags = "-Zmiri-disable-isolation -Zmiri-strict-provenance -Zmiri-ignore-leaks";

      # Deterministic (non-proptest) integration tests + crate unit tests (--lib),
      # run under Miri on every target. Keep in sync with tests/*.rs: add a new
      # `--test <name>` here UNLESS the file is a proptest_*.rs (those run only
      # natively via the cargo-test hook and under Miri in CI).
      deterministicMiriTests = builtins.concatStringsSep " " [
        "--lib"
        "--test allocator_comprehensive"
        "--test allocator_integration"
        "--test bytes"
        "--test dynamic_vec"
        "--test eco_map"
        "--test eco_set"
        "--test eco_str_referenced"
        "--test eco_string"
        "--test ecow_superset"
        "--test leak_tests"
        "--test serde_rkyv"
        "--test small_map"
        "--test small_set"
        "--test small_vec"
        "--test tests"
        "--test track_caller"
        "--test ub_guards"
        "--test vec_drain"
        "--test vec_io"
        "--test vec_splice"
      ];

      mkMiriHook = target: {
        enable = true;
        name = "cargo miri test (${target})";
        entry = toString (pkgs.writeShellScript "cargo-miri-${target}" ''
          export PATH="${rustToolchain}/bin:$PATH"
          export MIRIFLAGS="${miriFlags}"
          cargo miri test ${deterministicMiriTests} --target ${target}-unknown-linux-gnu --manifest-path Cargo.toml
        '');
        files = "\\.(rs|toml)$";
        pass_filenames = false;
        stages = [ "pre-push" ];
      };

      gitHooks = inputs.git-hooks.lib.${system}.run {
        src = ../../.;

        hooks = {
          # ── Formatting ────────────────────────────────────────────
          treefmt = {
            enable = true;
            package = treeFmtWrapper;
          };

          # ── Rust compilation ──────────────────────────────────────
          cargo-check = {
            enable = true;
            name = "cargo check";
            entry = toString (pkgs.writeShellScript "cargo-check" ''
              export PATH="${rustToolchain}/bin:$PATH"
              cargo check --manifest-path Cargo.toml
            '');
            files = "\\.(rs|toml)$";
            pass_filenames = false;
          };

          # ── Lockfile freshness ────────────────────────────────────
          # turbocow's cfg(hotpath) / cfg(loom) deps resolve into the Cargo.lock of
          # every crate that depends on turbocow, so a turbocow dependency change
          # leaves the tools/loom-tests + benches locks stale. Check all of them up
          # front — `cargo metadata --locked` resolves without compiling and errors
          # on a stale lock — so a stale lock fails the push with a clear "run cargo
          # update" message instead of a later hook silently rewriting it (which
          # tripped pre-commit's "files modified by hook"). fail_fast aborts the
          # expensive miri/test/loom hooks immediately when a lock is out of date.
          cargo-locks = {
            enable = true;
            name = "cargo lockfiles up to date";
            entry = toString (pkgs.writeShellScript "cargo-locks" ''
              export PATH="${rustToolchain}/bin:$PATH"
              rc=0
              for m in Cargo.toml tools/loom-tests/Cargo.toml benches/Cargo.toml; do
                if ! cargo metadata --locked --manifest-path "$m" --format-version 1 >/dev/null 2>&1; then
                  echo "✗ $m — Cargo.lock is stale; run: cargo update --manifest-path $m" >&2
                  rc=1
                fi
              done
              exit $rc
            '');
            always_run = true;
            pass_filenames = false;
            fail_fast = true;
            stages = [ "pre-push" ];
          };

          # ── Rust tests ────────────────────────────────────────────
          cargo-test = {
            enable = true;
            name = "cargo test";
            entry = toString (pkgs.writeShellScript "cargo-test" ''
              export PATH="${rustToolchain}/bin:$PATH"
              cargo test --manifest-path Cargo.toml
            '');
            files = "\\.(rs|toml)$";
            pass_filenames = false;
            stages = [ "pre-push" ];
          };

          # ── MIRI (UB detection, 4 architectures, deterministic-only) ─
          # x86_64 (LE64, SSE2 matcher) · aarch64 (LE64, scalar) · sparc64
          # (BE64) · i686 (LE32). Proptests run natively + in CI; see the
          # `mkMiriHook` notes above.
          cargo-miri-x86_64 = mkMiriHook "x86_64";
          cargo-miri-aarch64 = mkMiriHook "aarch64";
          cargo-miri-sparc64 = mkMiriHook "sparc64";
          cargo-miri-i686 = mkMiriHook "i686";

          # ── Loom (concurrency verification) ───────────────────────
          loom = {
            enable = true;
            name = "loom test";
            entry = toString (pkgs.writeShellScript "loom-test" ''
              export PATH="${rustToolchain}/bin:$PATH"
              cd tools/loom-tests
              # --locked: never rewrite Cargo.lock during the hook. turbocow's
              # cfg(hotpath) dep makes this lock heavy; a silent rewrite trips the
              # pre-commit "files modified" check and fails the hook. If the lock is
              # stale this fails loudly → run
              # `cargo update --manifest-path tools/loom-tests/Cargo.toml` and commit.
              RUSTFLAGS="--cfg loom" cargo test --release --locked
            '');
            files = "\\.(rs|toml)$";
            pass_filenames = false;
            stages = [ "pre-push" ];
          };
        };
      };
    in
    {
      _module.args.gitHooks = gitHooks;
    };
}
