# Loads the pinned nightly toolchain from rust-toolchain.toml and
# constructs a crane lib that overrides it. Re-exports both via
# `_module.args` so peer modules (packages, devshell, hooks) can
# consume them without re-importing the toolchain file.
#
# Also installs the `rust-overlay` and `llm-agents` overlays into the
# project's pkgs so `pkgs.rust-bin.*` (toolchain) and
# `pkgs.llm-agents.*` (claude-code, codex) resolve everywhere.
{ inputs, ... }:
{
  perSystem = { system, ... }:
    let
      pkgs = import inputs.nixpkgs {
        inherit system;
        config.allowUnfree = true;
        overlays = [
          inputs.rust-overlay.overlays.default
          inputs.llm-agents.overlays.default
          # Pin `pkgs.wild` to the 0.9.0 flake input (used as the linker
          # via clang's -fuse-ld=wild) instead of nixpkgs' wild, so the
          # linker version is stable across nixpkgs bumps.
          (_final: _prev: { wild = inputs.wild.packages.${system}.default; })
        ];
      };

      rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ../../rust-toolchain.toml;
      craneLib = (inputs.crane.mkLib pkgs).overrideToolchain rustToolchain;
    in
    {
      _module.args = {
        inherit pkgs rustToolchain craneLib;
      };
    };
}
