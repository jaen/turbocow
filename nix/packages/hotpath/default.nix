{ fetchFromGitHub
, lib
, pkgs
, rustPlatform
, symlinkJoin
, writers
, python3Packages
, ...
}:
let
  version = "0.11.0";

  src = fetchFromGitHub {
    owner = "pawurb";
    repo = "hotpath-rs";
    tag = "v${version}";
    hash = "sha256-EuaLHMzfjo8TdwalxWv5SAPlkVmixcl8IlGonwoMQD0=";
  };

  # CLI / TUI profiler binary.
  profiler = rustPlatform.buildRustPackage {
    pname = "hotpath-profiler";
    inherit version src;

    # Upstream doesn't ship Cargo.lock — patch it in.
    cargoPatches = [ ./add-cargo-lock.patch ];
    cargoHash = "sha256-defKHjBiqQuH0eXT+Nq7kU8tQKLiU1EdLS9blqAZDYw=";

    # Only build the hotpath binary (CLI/TUI), not examples.
    cargoBuildFlags = [ "--bin" "hotpath" ];

    buildFeatures = [ "tui" ];

    # Tests require a running profiled application.
    doCheck = false;

    meta.mainProgram = "hotpath";
  };

  # Stdio MCP server that forwards to whichever HTTP-MCP endpoint
  # `hotpath` is serving (named endpoints configurable via
  # `.hotpath-proxy.yaml`). The script is vendored next to this
  # derivation (project-agnostic: it keys its config lookup off
  # `REPOSITORY_ROOT`, not a project-specific var) so the package is
  # self-contained and doesn't reach back into the project tree.
  proxy = writers.writePython3Bin "hotpath-mcp-proxy"
    {
      flakeIgnore = [ "E501" "E266" ];
      libraries = [ python3Packages.pyyaml ];
    }
    (builtins.readFile ./hotpath-mcp-proxy.py);
in
symlinkJoin {
  name = "hotpath-${version}";
  paths = [ profiler proxy ];

  meta = {
    description = "Rust performance profiler (CLI/TUI) + stdio MCP proxy";
    homepage = "https://github.com/pawurb/hotpath-rs";
    license = lib.licenses.mit;
    mainProgram = "hotpath";
  };
}
