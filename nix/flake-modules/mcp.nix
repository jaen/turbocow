# Evaluate the shared `mcp` module (nix/mcp/module.nix +
# nix/mcp/servers.nix) under flake-parts and expose:
#
#   * `apps.write-mcp-configs` — `nix run .#write-mcp-configs` writes
#     `.mcp.json` + `.cursor/mcp.json` into the working tree. Use this
#     when you want to refresh the configs without going through
#     direnv (e.g. after editing nix/mcp/servers.nix).
#   * `_module.args.mcpServerList` — the deduped list of packages
#     backing the enabled servers; `devshell.nix` appends it to the
#     shell's `packages` so the MCP server binaries are on PATH.
#   * `_module.args.mcpWriteConfigs` — the writer derivation reused
#     from the shellHook so configs stay in sync on `direnv reload`.
{ lib, ... }:
{
  perSystem =
    { config
    , pkgs
    , mcp-servers
    , ...
    }:
    let
      mcpGen = import ../mcp/generate.nix { inherit lib; };

      # Run the module via `lib.evalModules` so we get the same
      # option-type checking + defaults as a NixOS-style module.
      mcpEval = lib.evalModules {
        modules = [
          ../mcp/module.nix
          ../mcp/servers.nix
          { _module.args = { inherit pkgs mcp-servers; }; }
        ];
      };

      mcp = mcpEval.config.mcp;

      configs = mcpGen.mkAllConfigs {
        inherit (mcp) servers hosts root;
      };

      # Shell script that writes each generated config to its path,
      # creating parent directories as needed and pretty-printing the
      # JSON so the on-disk file stays human-readable.
      writeAllConfigs = pkgs.writeShellApplication {
        name = "write-mcp-configs";
        runtimeInputs = [ pkgs.coreutils pkgs.jq ];
        text = lib.concatStringsSep "\n" (lib.mapAttrsToList
          (path: text:
            let
              jsonDrv = pkgs.writeText "mcp-config-${baseNameOf path}.json" text;
            in
            ''
              mkdir -p "$(dirname ${lib.escapeShellArg path})"
              jq --indent 2 . ${jsonDrv} > ${lib.escapeShellArg path}
              echo "wrote ${path}"
            '')
          configs);
      };
    in
    {
      _module.args = {
        mcpModule = mcp;
        mcpServerList = mcpGen.serverPackages mcp.servers;
        mcpWriteConfigs = writeAllConfigs;
      };

      apps.write-mcp-configs = {
        type = "app";
        program = lib.getExe writeAllConfigs;
      };

      packages.write-mcp-configs = writeAllConfigs;
    };
}
