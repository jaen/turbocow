{ lib, ... }:
let
  inherit (lib) mkOption mkEnableOption types;
in
{
  options.mcp = {
    enable = mkEnableOption "MCP server inventory & client-config generation";

    servers = mkOption {
      description = ''
        Inventory of MCP servers. Single source of truth consumed by
        the flake-parts side (nix/flake-modules/mcp.nix). Each entry
        becomes one `mcpServers.<name>` block in the emitted
        `.mcp.json` / `.cursor/mcp.json`.
      '';
      default = { };
      type = types.attrsOf (types.submodule ({ name, ... }: {
        options = {
          enable = mkOption {
            type = types.bool;
            default = true;
            description = "Include this server in the emitted config(s).";
          };

          package = mkOption {
            type = types.nullOr types.package;
            default = null;
            description = ''
              Package providing the server binary. Added to the dev
              shell PATH when set so the `command` resolves at
              runtime. Optional — set this null and supply an
              absolute path in `command` to register an externally
              installed server without putting it in the shell.
            '';
          };

          command = mkOption {
            type = types.str;
            description = ''
              Binary name (resolved via PATH) or absolute path. Defaults
              to the server's slug — override only when the package's
              main program differs.
            '';
            default = name;
          };

          args = mkOption {
            type = types.listOf types.str;
            default = [ ];
            description = "Positional arguments passed to the server binary.";
          };

          env = mkOption {
            type = types.attrsOf types.str;
            default = { };
            description = "Environment variables set when the host spawns the server.";
          };

          hosts = mkOption {
            type = types.listOf (types.enum [ "claude" "cursor" ]);
            default = [ "claude" "cursor" ];
            description = ''
              Which clients emit this server into. Used to keep
              host-specific servers out of the other client's config.
            '';
          };

          # Cursor-specific tool-level filtering — passed through into
          # `.cursor/mcp.json` only; stripped from `.mcp.json` (Claude
          # Code rejects unknown fields with a warning).
          disabled = mkOption {
            type = types.bool;
            default = false;
            description = "Cursor: hide this server from the tool picker without removing it.";
          };

          disabledTools = mkOption {
            type = types.listOf types.str;
            default = [ ];
            description = ''
              Cursor: per-tool deny-list. Use this when a server
              exposes too many tools and you want to keep a subset hot.
            '';
          };

          useDirenv = mkOption {
            type = types.bool;
            default = true;
            description = ''
              Wrap the command in `direnv exec <root> <command>` so
              the server inherits the project's nix-built PATH +
              environment regardless of how the MCP host was launched.
              Disable for servers already on the global PATH or
              pointed to by absolute path.
            '';
          };
        };
      }));
    };

    hosts = mkOption {
      description = ''
        Map of client name → repo-relative output path. The defaults
        match Claude Code's project-scope (`./.mcp.json`) and
        Cursor's project-scope (`./.cursor/mcp.json`) conventions.
      '';
      type = types.attrsOf types.str;
      default = {
        claude = ".mcp.json";
        cursor = ".cursor/mcp.json";
      };
    };

    root = mkOption {
      type = types.str;
      default = ".";
      description = ''
        Project root, passed as the second argument to
        `direnv exec ROOT <command>`. `.` works for clients launched
        from the project directory.
      '';
    };
  };
}
