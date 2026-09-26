# Composes the project dev shell on top of crane's `devShell` (which
# brings in the right rust toolchain + cargo-build env). Pulls MCP
# server binaries onto PATH via `mcpServerList` (derived from
# nix/mcp/servers.nix), and runs the MCP config writer on shell entry
# so `.mcp.json` / `.cursor/mcp.json` stay in sync with the inventory.
{ ... }:
{
  perSystem =
    { pkgs
    , rustToolchain
    , mcpServerList
    , mcpWriteConfigs
    , mcp-servers
    , turbocowDev
    , treeFmt
    , gitHooks
    , ...
    }:
    let
      bench-parallel = pkgs.writeShellScriptBin "bench-parallel"
        (builtins.readFile ../../scripts/bench-parallel);

      # `cargo loom` / `cargo hotpath` — wrappers for the cfg-gated builds so you
      # don't have to set RUSTFLAGS by hand. Each scopes its `--cfg` to that one
      # invocation; a global RUSTFLAGS would instead force the cfg into EVERY
      # build (pulling hotpath's whole stack / routing all atomics through loom).
      cargo-loom = pkgs.writeShellScriptBin "cargo-loom" ''
        # Run the loom concurrency model-check (the standalone tools/loom-tests
        # crate) with `--cfg loom` set for you. Args pass through to `cargo test`.
        root="''${WORKSPACE_ROOT:-$(${pkgs.git}/bin/git rev-parse --show-toplevel 2>/dev/null || pwd)}"
        exec env RUSTFLAGS="--cfg loom -C link-arg=-fuse-ld=wild" \
          cargo test --release --manifest-path "$root/tools/loom-tests/Cargo.toml" "$@"
      '';

      cargo-hotpath = pkgs.writeShellScriptBin "cargo-hotpath" ''
        # Profile the benchmarks (the standalone benches/ crate) with the hotpath
        # profiler enabled (`--cfg hotpath`) under the `profiling` profile, e.g.
        # `cargo hotpath --bench string_comparison`. For a non-bench target, set
        # RUSTFLAGS="--cfg hotpath" and run cargo yourself.
        root="''${WORKSPACE_ROOT:-$(${pkgs.git}/bin/git rev-parse --show-toplevel 2>/dev/null || pwd)}"
        exec env RUSTFLAGS="--cfg hotpath -C link-arg=-fuse-ld=wild" \
          cargo bench --manifest-path "$root/benches/Cargo.toml" --profile profiling "$@"
      '';

      # Rust-side dev tools. `mcp-servers.hotpath` puts both `hotpath`
      # (the profiler CLI/TUI) and `hotpath-mcp-proxy` (the stdio MCP
      # proxy) on PATH — the proxy is also reached via `mcpServerList`
      # below, but pulling the bundle in once keeps the profiler CLI
      # available even if MCP wiring is disabled later.
      rustDevTools = with pkgs; [
        bacon
        cargo-bloat
        cargo-edit
        cargo-expand
        cargo-flamegraph
        cargo-nextest
        cargo-outdated
        cargo-watch
        clang
        wild
        samply
        heaptrack
        gdb
        mcp-servers.hotpath
      ];

      commonDevTools = with pkgs; [
        ast-grep
        bat
        direnv
        fd
        git
        jq
        nixd
        ripgrep
      ];

      # numtide's nix-ai-tools renamed claude-code-acp -> claude-agent-acp.
      llmAgents = with pkgs.llm-agents; [
        claude-code
        claude-agent-acp
        codex
        codex-acp
      ];
    in
    {
      devShells.default = turbocowDev.overrideAttrs (oldAttrs: {
        shellHook = (oldAttrs.shellHook or "") + gitHooks.shellHook + ''
          export WORKSPACE_ROOT="''${WORKSPACE_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"
          # hotpath-mcp-proxy locates .hotpath-proxy.yaml via REPOSITORY_ROOT
          # (the project-agnostic var the vendored script keys off).
          export REPOSITORY_ROOT="''${REPOSITORY_ROOT:-$WORKSPACE_ROOT}"

          # Regenerate .mcp.json + .cursor/mcp.json from
          # nix/mcp/servers.nix so the on-disk configs always match
          # the inventory after a `direnv reload`.
          ${pkgs.lib.getExe mcpWriteConfigs} >/dev/null || true
        '';

        buildInputs = (oldAttrs.buildInputs or [ ])
          ++ rustDevTools
          ++ commonDevTools
          ++ llmAgents
          ++ mcpServerList
          ++ [
          treeFmt.config.build.wrapper
          bench-parallel
          cargo-loom
          cargo-hotpath
        ];
      });
    };
}
