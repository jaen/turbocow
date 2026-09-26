{ lib
, pkgs
, inputs
, system
, rustToolchain
, ...
}:
{
  # Rust-side MCP servers (built from source).
  rust-analyzer-mcp = pkgs.callPackage ./rust-analyzer-mcp { };
  crates-lsp = pkgs.callPackage ./crates-lsp { };

  # Python-backed MCP servers (uv2nix workspaces).
  ast-grep-mcp = pkgs.callPackage ./ast-grep-mcp { inherit inputs; };
  mcp-server-tree-sitter = pkgs.callPackage ./mcp-server-tree-sitter { inherit inputs; };

  # Built locally against our own pkgs so it shares glibc + openssl
  # with the rest of the dev shell. Upstream's flake pins an older
  # nixpkgs which surfaces as `libssl.so.3 not found` at runtime when
  # we use its packaged binary.
  rust-docs-mcp = pkgs.callPackage ./rust-docs-mcp { };

  # Profiler CLI/TUI + stdio↔HTTP MCP proxy bundled in one derivation
  # (exposes both `hotpath` and `hotpath-mcp-proxy` binaries). Lives
  # outside this folder so non-MCP consumers (e.g. the dev shell) can
  # still reach the profiler directly via `nix/packages/hotpath`.
  hotpath = pkgs.callPackage ../hotpath { };

  # Stdio MCP server answering structured queries about samply Firefox
  # Profiler JSON traces. Source vendored under ./samply-mcp/source;
  # the `samply` CLI itself comes from nixpkgs via the dev shell.
  samply-mcp = pkgs.callPackage ./samply-mcp { };
}

