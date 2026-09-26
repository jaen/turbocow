# rust-docs-mcp, built from source against the project's nixpkgs.
#
# We deliberately don't consume upstream's flake (and don't carry it
# as a flake input either) for two reasons:
#
#  * Upstream's flake pins its own nixpkgs at a glibc that's typically
#    a release behind ours. The resulting binary ends up loading
#    libc.so.6 from glibc-2.40 while LD_LIBRARY_PATH from the dev
#    shell pulls libdl.so.2 / libpthread.so.0 from glibc-2.42 — and
#    the newer libpthread requires `GLIBC_ABI_DT_X86_64_PLT`, a
#    symbol that doesn't exist in the older libc. End result: the
#    binary segfaults the moment Claude/Cursor tries to spawn it.
#
#  * Upstream's flake has been flaky in pure consumers: PR #46 sets
#    `fenix.fromToolchainFile { sha256 = ""; }` (TOFU), and PR #47
#    introduced `pkgs.lib.isLinux` which is not a real attribute. By
#    fetching the source ourselves we sidestep their flake entirely
#    and can pin/patch the tip independently.
#
# Rebuild trigger: bump `rev` + `hash`, optionally bump the nightly
# pin to whatever `PREFERRED_TOOLCHAIN` is in rust-docs-mcp/src/
# rustdoc.rs at that commit.
{ lib
, pkgs
, fetchFromGitHub
, makeRustPlatform
, makeWrapper
, openssl
, pkg-config
, git
, coreutils
, bash
, ...
}:
let
  rev = "adc2345d1a7bcd442dace64ff1cb107b1723d8fc";

  src = fetchFromGitHub {
    owner = "snowmead";
    repo = "rust-docs-mcp";
    inherit rev;
    hash = "sha256-6Q+l5vBYrWHIKMaGsSupuX1Gr1h0fyv6AZPTrVPkIBs=";
  };

  # rust-docs-mcp's binary shells out to `cargo +<nightly> rustdoc
  # --output-format json` to produce the rustdoc-json the MCP serves.
  # That requires both a nightly with `rust-docs-json` available AND
  # a `rustdoc-types` crate version whose `FORMAT_VERSION` matches the
  # `format_version` field emitted by that nightly's rustdoc-json.
  #
  # Upstream's tip pins `rustdoc-types = 0.53.0` (FORMAT_VERSION=53),
  # which only matches nightlies in roughly the 2025-06-24..2025-07-15
  # window. We patch the dep up to 0.57.3 (FORMAT_VERSION=57, latest
  # published) below, which lets it track a current nightly (pinned to
  # `nightlyDate` here). The API surface rust-docs-mcp touches
  # (Crate/Id/Item/ItemEnum/Visibility/Generics/Type/Struct/StructKind/
  # Enum/ItemSummary) compiles unchanged across 0.53 → 0.57. See
  # `postPatch` + `Cargo.lock.patched`.
  nightlyDate = "2026-05-16";
  rustNightly = pkgs.rust-bin.nightly.${nightlyDate}.default.override {
    extensions = [
      "rust-src"
      "rust-docs-json"
      "rustfmt"
      "clippy"
    ];
  };

  rustPlatformNightly = makeRustPlatform {
    cargo = rustNightly;
    rustc = rustNightly;
  };

  # The binary uses rustup's `cargo +toolchain rustdoc ...` proxy
  # syntax. Nix-built cargo/rustc/rustdoc don't recognise +toolchain,
  # so each tool is wrapped to drop any leading `+nightly*` / `+stable*`
  # / `+beta*` arg before exec'ing the real binary.
  mkToolShim =
    name:
    pkgs.writeShellScriptBin name ''
      args=()
      for arg in "$@"; do
        case "$arg" in +nightly*|+stable*|+beta*) ;; *) args+=("$arg") ;; esac
      done
      exec ${rustNightly}/bin/${name} "''${args[@]}"
    '';

  # Minimal rustup stand-in that satisfies the few subcommands the
  # rust-docs-mcp binary probes (`toolchain list`, `which`, `show`,
  # `run`). Lets us avoid pulling rustup itself into the closure.
  rustupShim = pkgs.writeShellScriptBin "rustup" ''
    case "$1" in
      toolchain)
        echo "nightly-${nightlyDate}-x86_64-unknown-linux-gnu (default)"
        ;;
      which)
        command -v "''${2:-rustc}" 2>/dev/null || echo "rustc"
        ;;
      show)
        echo "Default host: x86_64-unknown-linux-gnu"
        echo ""
        echo "installed toolchains"
        echo "--------------------"
        echo "nightly-${nightlyDate}-x86_64-unknown-linux-gnu (default)"
        ;;
      run)
        shift; shift
        exec "$@"
        ;;
      *)
        exit 0
        ;;
    esac
  '';

  rustToolShims = pkgs.symlinkJoin {
    name = "rust-tool-shims";
    paths = [
      (mkToolShim "cargo")
      (mkToolShim "rustdoc")
      (mkToolShim "rustc")
      rustupShim
    ];
  };

  unwrapped = rustPlatformNightly.buildRustPackage {
    pname = "rust-docs-mcp";
    version = "0.1.1-tip-${builtins.substring 0 7 rev}";

    inherit src;

    # Bump `rustdoc-types` from 0.53.0 → 0.57.3 so the runtime
    # FORMAT_VERSION check matches a current nightly's rustdoc-json
    # output. Cargo.toml is rewritten in `postPatch`; the matching
    # Cargo.lock is shipped alongside this derivation.
    cargoLock = {
      lockFile = ./Cargo.lock.patched;
    };

    postPatch = ''
      substituteInPlace rust-docs-mcp/Cargo.toml \
        --replace-fail 'rustdoc-types = { version = "0.53.0"' \
                       'rustdoc-types = { version = "0.57.3"'
      # Align the source's hardcoded toolchain constant with the
      # nightly we actually ship; otherwise `doctor` reports a
      # confusingly stale date even though probing succeeds.
      substituteInPlace rust-docs-mcp/src/rustdoc.rs \
        --replace-fail 'pub const PREFERRED_TOOLCHAIN: &str = "nightly-2025-06-24";' \
                       'pub const PREFERRED_TOOLCHAIN: &str = "nightly-${nightlyDate}";'
      # Keep source's lockfile in sync with the vendored one so the
      # cargoSetupPostPatchHook consistency check passes.
      cp ${./Cargo.lock.patched} Cargo.lock
    '';

    # Workspace has two members (rust-docs-mcp + cargo-modules); we
    # only want the server binary.
    buildAndTestSubdir = "rust-docs-mcp";

    nativeBuildInputs = [ pkg-config ];
    buildInputs = [ openssl ];

    # Upstream's test suite hits the network (downloads crates for
    # rustdoc-json regression checks) — won't run in the sandbox.
    doCheck = false;

    meta = with lib; {
      description = "MCP server exposing rustdoc-json for arbitrary crates";
      homepage = "https://github.com/snowmead/rust-docs-mcp";
      license = licenses.mit;
      mainProgram = "rust-docs-mcp";
    };
  };
in
pkgs.runCommand "rust-docs-mcp-wrapped"
{
  nativeBuildInputs = [ makeWrapper ];
  meta = unwrapped.meta;
  passthru = { inherit unwrapped rustNightly; };
}
  ''
    mkdir -p $out/bin
    makeWrapper ${unwrapped}/bin/rust-docs-mcp $out/bin/rust-docs-mcp \
      --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath [ openssl ]} \
      --prefix PATH : ${
        lib.makeBinPath [
          rustToolShims
          git
          coreutils
          bash
        ]
      }
  ''
