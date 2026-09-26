{
  description = "Turbocow — clone-on-write vector and string with allocator support";

  inputs = {
    systems.url = "github:nix-systems/default";

    nix-systems-default = {
      url = "https://git.jaen.me/mirrors/nix-systems-default/archive/main.tar.gz";
    };

    nixpkgs = {
      url = "https://git.jaen.me/mirrors/nixpkgs/archive/nixos-unstable.tar.gz";
    };

    nixpkgs-lib = {
      url = "https://git.jaen.me/mirrors/nixpkgs.lib/archive/master.tar.gz";
    };

    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs-lib";
    };

    # Auto-discovers flake-parts modules in a directory tree.
    import-tree = {
      url = "https://git.jaen.me/mirrors/nix-import-tree/archive/main.tar.gz";
    };

    rust-overlay = {
      url = "https://git.jaen.me/mirrors/rust-overlay/archive/master.tar.gz";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    crane = {
      url = "https://git.jaen.me/mirrors/crane/archive/master.tar.gz";
    };

    # Pinned wild linker (0.9.0) — the actual linker invoked via clang's
    # `-fuse-ld=wild` (see .cargo/config.toml). Pinned to a release rather
    # than tracking nixpkgs so the linker stays stable across nixpkgs
    # bumps (matches eiga). Built against the project's own nixpkgs/crane —
    # rustc there is new enough (wild 0.9.0 needs Rust >= 1.94).
    wild = {
      url = "https://git.jaen.me/mirrors/wild-linker/archive/0.9.0.tar.gz";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.crane.follows = "crane";
    };

    # numtide renamed llm-agents.nix -> nix-ai-tools; the overlay still
    # exposes the `llm-agents` attrset, so the input name is kept for
    # consumers. Pulled from the git.jaen.me mirror for consistency with
    # the other inputs.
    llm-agents = {
      url = "https://git.jaen.me/mirrors/nix-ai-tools/archive/main.tar.gz";
    };

    # Preserved: `default.nix` / `shell.nix` / `nix/flake-compat.nix`
    # delegate through flake-compat so non-flake `nix-build` /
    # `nix-shell` consumers keep working after the flake-parts
    # migration.
    flake-compat = {
      url = "https://git.jaen.me/mirrors/lix-flake-compat/archive/main.tar.gz";
    };

    treefmt-nix = {
      url = "https://git.jaen.me/mirrors/treefmt-nix/archive/main.tar.gz";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    gitignore-nix = {
      url = "https://git.jaen.me/mirrors/hercules-ci-gitignore.nix/archive/master.tar.gz";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    git-hooks = {
      url = "https://git.jaen.me/mirrors/cachix-git-hooks.nix/archive/master.tar.gz";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-compat.follows = "flake-compat";
      inputs.gitignore.follows = "gitignore-nix";
    };

    # Python build system dependencies (for MCP servers backed by
    # uv2nix workspaces: ast-grep-mcp, mcp-server-tree-sitter).
    pyproject-nix = {
      url = "https://git.jaen.me/mirrors/pyproject.nix/archive/master.tar.gz";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    uv2nix = {
      url = "https://git.jaen.me/mirrors/uv2nix/archive/master.tar.gz";
      inputs = {
        pyproject-nix.follows = "pyproject-nix";
        nixpkgs.follows = "nixpkgs";
      };
    };

    pyproject-build-systems = {
      url = "https://git.jaen.me/mirrors/pyproject-nix-build-system-pkgs/archive/master.tar.gz";
      inputs = {
        pyproject-nix.follows = "pyproject-nix";
        uv2nix.follows = "uv2nix";
        nixpkgs.follows = "nixpkgs";
      };
    };
  };

  outputs = inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [ "x86_64-linux" "aarch64-linux" ];

      # Auto-import every *.nix file under nix/flake-modules/. Order
      # is irrelevant — flake-parts merges modules and resolves
      # `_module.args` dependencies at evaluation time.
      imports = [ (inputs.import-tree ./nix/flake-modules) ];
    };
}
