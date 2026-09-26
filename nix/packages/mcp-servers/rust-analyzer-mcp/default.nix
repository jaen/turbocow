{ fetchFromGitHub
, lib
, rustPlatform
, rust-analyzer
, makeWrapper
, ...
}:
# Tracks the open `better-mcp-series` branch on
# `the-real-wizard-of-oz/rust-analyzer-mcp` (PR #18 against
# zeenix/rust-analyzer-mcp). That branch expands the upstream 10-tool
# LSP shim into a 36-tool service with cancellation, auto-restart,
# multi-workspace support, and token-conscious output shaping. Bump
# `rev` when the PR refreshes; recompute hashes via a TOFU build.
rustPlatform.buildRustPackage {
  pname = "rust-analyzer-mcp";
  version = "0.2.0-pr18-90f8aee";

  src = fetchFromGitHub {
    owner = "the-real-wizard-of-oz";
    repo = "rust-analyzer-mcp";
    rev = "90f8aee2b47fe9b28c6e8ec86de3805b5ab291ed";
    hash = "sha256-hT8ZGGnEV0b9PHI28bJL7TJ02aGvT8mzcjjaVVqZ3S0=";
  };

  cargoHash = "sha256-8rwSso7nNOTWXgdUSp8Hdd/DZlrVfNVt2voGO1sG4VM=";

  nativeBuildInputs = [ makeWrapper ];

  # Tests rely on a live rust-analyzer process + workspace fixtures.
  doCheck = false;

  postInstall = ''
    wrapProgram $out/bin/rust-analyzer-mcp \
      --prefix PATH : ${lib.makeBinPath [ rust-analyzer ]}
  '';

  meta = {
    description = "MCP server for rust-analyzer (the-real-wizard-of-oz/better-mcp-series — 36 tools, concurrent, multi-workspace)";
    homepage = "https://github.com/zeenix/rust-analyzer-mcp/pull/18";
    license = lib.licenses.mit;
    mainProgram = "rust-analyzer-mcp";
  };
}
