# Single source of truth for the MCP servers wired into turbocow's
# dev shell. `nix/flake-modules/mcp.nix` evaluates this against
# `nix/mcp/module.nix` to emit `.mcp.json` / `.cursor/mcp.json` and
# to derive the package list added to the dev shell PATH.
{ mcp-servers, ... }:
{
  mcp.enable = true;
  mcp.root = ".";

  mcp.servers = {
    # ── Language-server-backed code intelligence ──────────────────

    # rust-analyzer over MCP — the-real-wizard-of-oz/better-mcp-series
    # fork tracking PR #18 against zeenix/rust-analyzer-mcp. 36 tools,
    # cancellation, auto-restart, multi-workspace.
    rust-analyzer = {
      package = mcp-servers.rust-analyzer-mcp;
      command = "rust-analyzer-mcp";
    };

    # crates-lsp is a Language Server, not an MCP server — it
    # doesn't expose MCP tools to Claude / Cursor on its own. To use
    # it from an MCP host it has to be wrapped via mcp-language-server
    # (`mcp-language-server --workspace . --lsp crates-lsp`), and even
    # then the LSP-bridged surface is flaky (see carol-new's notes:
    # "LSP wrapper seems problematic as always"). Left here in case
    # you want to flip it on after wiring the wrapper.
    crates-lsp = {
      enable = false;
      package = mcp-servers.crates-lsp;
      command = "crates-lsp";
    };

    # ── Syntactic search / structural rewriting ──────────────────

    # ast-grep MCP — pattern matching across Rust, Nix, and friends.
    # Binary name is `ast-grep-server` (not `ast-grep-mcp`).
    ast-grep = {
      package = mcp-servers.ast-grep-mcp;
      command = "ast-grep-server";
    };

    # tree-sitter-backed structural queries.
    tree-sitter = {
      package = mcp-servers.mcp-server-tree-sitter;
      command = "mcp-server-tree-sitter";
    };

    # ── Documentation / library search ────────────────────────────

    # rustdoc-json-backed crate docs lookup; the package's own wrapper
    # puts a nightly rustc on PATH so the server can render docs.
    rust-docs = {
      package = mcp-servers.rust-docs-mcp;
      command = "rust-docs-mcp";
    };

    # ── Profiling ─────────────────────────────────────────────────

    # Stdio MCP that forwards to whichever HTTP endpoint a running
    # `hotpath` profiler is serving (named endpoints in
    # `.hotpath-proxy.yaml`). The proxy binary is bundled inside the
    # `hotpath` package alongside the profiler CLI — one derivation,
    # two binaries (hotpath + hotpath-mcp-proxy).
    hotpath = {
      package = mcp-servers.hotpath;
      command = "hotpath-mcp-proxy";
    };

    # Stdio MCP that answers structured queries about samply Firefox
    # Profiler JSON traces (produced by `samply record …` against
    # benchmarks / examples). Complements hotpath: hotpath is live
    # in-process metrics, samply is post-hoc sampled stack profiles.
    samply = {
      package = mcp-servers.samply-mcp;
      command = "samply-mcp";
    };
  };
}
