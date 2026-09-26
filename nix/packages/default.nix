{ pkgs
, craneLib
, rustToolchain
,
}:
let
  rawSrc = craneLib.path ../..;
  src = pkgs.lib.cleanSourceWith {
    src = rawSrc;
    filter = path: type:
      (craneLib.filterCargoSources path type)
      || (type == "directory")
      # The crate root embeds README.md via `include_str!`, so the
      # doctest harness needs it present in the sandbox — crane's
      # cargo-source filter would otherwise strip all *.md.
      || (pkgs.lib.hasSuffix "/README.md" path);
  };

  commonArgs = {
    inherit src;
    strictDeps = true;
    # Read pname/version from Cargo.toml so the derivation name never drifts
    # from the crate version.
    inherit (craneLib.crateNameFromCargoToml { cargoToml = ../../Cargo.toml; }) pname version;

    # `.cargo/config.toml` sets `linker = "clang"` and
    # `-Clink-arg=-fuse-ld=wild`, so the crane sandbox needs both clang
    # (the linker driver) and wild (the actual linker) on PATH — without
    # them linking fails with `linker 'clang' not found`.
    nativeBuildInputs = [ rustToolchain pkgs.clang pkgs.wild ];
  };

  cargoArtifacts = craneLib.buildDepsOnly commonArgs;

  # tools/loom-tests is a standalone crate (its own Cargo.lock) that
  # depends on turbocow with the `loom` feature. It exists precisely to
  # isolate the `--cfg loom` build: forcing `--cfg loom` on the whole
  # root tree breaks dev-deps like ecow / lean_string that carry their
  # own `#[cfg(loom)]` code but don't depend on the `loom` crate. As a
  # normal (non-dev) dependent of turbocow it only pulls the library's
  # deps, so loom stays scoped to turbocow + loom-tests.
  loomVendor = craneLib.vendorCargoDeps {
    cargoLock = ../../tools/loom-tests/Cargo.lock;
  };
in
rec {
  # ── Build ────────────────────────────────────────────────────────
  default = craneLib.buildPackage (commonArgs // {
    inherit cargoArtifacts;
  });

  deps-only = cargoArtifacts;

  # ── Checks ───────────────────────────────────────────────────────
  check = craneLib.buildPackage (commonArgs // {
    inherit cargoArtifacts;
    pname = "turbocow-check";
    doCheck = true;
    cargoExtraArgs = "--all-targets";
  });

  clippy = craneLib.cargoClippy (commonArgs // {
    inherit cargoArtifacts;
    cargoClippyExtraArgs = "--all-targets -- --deny warnings -A dead-code -A unused-imports";
  });

  test = craneLib.cargoNextest (commonArgs // {
    inherit cargoArtifacts;
    partitions = 1;
    partitionType = "count";
  });

  # NOTE: MIRI cannot run in the nix sandbox:
  # - Needs network to fetch sysroot deps (hashbrown, etc.)
  # - cargo miri setup produces non-deterministic .rlib output,
  #   so a fixed-output derivation (FOD) is not possible either.
  # Run manually: MIRIFLAGS="-Zmiri-ignore-leaks" cargo miri test

  loom = craneLib.mkCargoDerivation {
    inherit src;
    strictDeps = true;
    pname = "turbocow-loom-tests";
    version = "0.0.0";
    # Standalone crate with its own lock + the `loom` dep; build its deps
    # inline (don't reuse turbocow's root artifacts) and vendor its lock.
    cargoArtifacts = null;
    cargoVendorDir = loomVendor;
    nativeBuildInputs = [ rustToolchain pkgs.clang pkgs.wild ];
    buildPhaseCargoCommand = ''
      RUSTFLAGS="--cfg loom" cargo test --release --manifest-path tools/loom-tests/Cargo.toml
    '';
  };

  # ── Dev shell ────────────────────────────────────────────────────
  devShell = craneLib.devShell {
    inputsFrom = [ default ];
    CARGO_INCREMENTAL = "1";
    RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
    RUST_BACKTRACE = 1;
  };
}
