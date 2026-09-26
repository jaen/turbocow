# Pure helpers — no `pkgs`, no IO — that turn the `mcp.servers`
# option value into one JSON document per host. Consumed by
# `nix/flake-modules/mcp.nix`.
{ lib }:
let
  inherit (lib) filterAttrs mapAttrs;

  # Render one server entry as the value half of `mcpServers.<name>`.
  # `host` drives field selection — Cursor accepts the extra
  # `disabled` / `disabledTools` fields; Claude Code rejects unknown
  # keys with a warning so we omit them there.
  serverToJson = host: server:
    let
      direnvRoot = server.root or ".";
      base = {
        command =
          if server.useDirenv
          then "direnv"
          else server.command;
        args =
          if server.useDirenv
          then [ "exec" direnvRoot server.command ] ++ server.args
          else server.args;
      } // (lib.optionalAttrs (server.env != { }) { inherit (server) env; });

      cursorExtras = lib.optionalAttrs (host == "cursor") (
        (lib.optionalAttrs server.disabled { inherit (server) disabled; })
        // (lib.optionalAttrs (server.disabledTools != [ ]) { inherit (server) disabledTools; })
      );
    in
    base // cursorExtras;

  # Produce the full `.mcp.json` / `.cursor/mcp.json` text for one host.
  mkHostJson = host: servers:
    let
      visible = filterAttrs
        (_: s: s.enable && (builtins.elem host s.hosts))
        servers;
      body = { mcpServers = mapAttrs (_: serverToJson host) visible; };
    in
    builtins.toJSON body + "\n";

  # Map { host → output-path } × server inventory → { output-path → text },
  # ready to write via an app.
  mkAllConfigs = { servers, hosts, root ? "." }:
    lib.listToAttrs (lib.mapAttrsToList
      (host: path: lib.nameValuePair path (
        mkHostJson host (mapAttrs (_: s: s // { inherit root; }) servers)
      ))
      hosts);

  # Pull every non-null server.package out for `mkShell.packages` —
  # keeps the inventory the single source of truth for "which MCP
  # binaries are on PATH".
  serverPackages = servers:
    lib.unique (
      lib.filter (p: p != null)
        (lib.mapAttrsToList (_: s: s.package) (filterAttrs (_: s: s.enable) servers))
    );
in
{
  inherit serverToJson mkHostJson mkAllConfigs serverPackages;
}
