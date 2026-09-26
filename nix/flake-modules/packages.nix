# Crane-built turbocow library + checks, plus re-exports of the
# locally-built MCP server set. The `mcp-servers` attrset is also
# re-exposed via `_module.args` so `mcp.nix` and `devshell.nix` can
# consume it.
{ inputs, ... }:
{
  perSystem = { pkgs, system, craneLib, rustToolchain, ... }:
    let
      mcp-servers = pkgs.callPackage ../packages/mcp-servers {
        inherit inputs system rustToolchain;
      };

      turbocowPkgs = import ../packages {
        inherit pkgs craneLib rustToolchain;
      };
    in
    {
      _module.args = { inherit mcp-servers; };

      packages = {
        default = turbocowPkgs.default;
        deps-only = turbocowPkgs.deps-only;
      } // {
        inherit (mcp-servers)
          rust-analyzer-mcp
          crates-lsp
          ast-grep-mcp
          mcp-server-tree-sitter
          rust-docs-mcp
          hotpath
          samply-mcp
          ;
      };

      checks = {
        inherit (turbocowPkgs) check clippy test loom;
      };

      # Re-exported so `devshell.nix` can wire it into the dev shell.
      _module.args.turbocowDev = turbocowPkgs.devShell;
    };
}
