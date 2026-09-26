{ self, inputs, ... }:
{
  perSystem = { pkgs, rustToolchain, ... }:
    let
      treeFmt = inputs.treefmt-nix.lib.evalModule pkgs {
        projectRootFile = "flake.nix";

        programs = {
          nixpkgs-fmt.enable = true;
          rustfmt = {
            enable = true;
            package = rustToolchain;
          };
          shfmt.enable = true;
          shellcheck.enable = true;
          taplo.enable = true;
          yamlfmt.enable = true;
        };

        settings = {
          global.excludes = [
            "*.patch"
            "*.json"
            "*.md"
            ".gitignore"
            "flake.lock"
            "treefmt.toml"
            # Vendored upstream samply-mcp source — kept byte-for-byte so
            # it can be re-copied from upstream (see the package's
            # default.nix). Don't let the project's (nightly) rustfmt
            # rewrite third-party code.
            "nix/packages/mcp-servers/samply-mcp/source/**"
          ];
        };
      };
    in
    {
      _module.args.treeFmt = treeFmt;

      formatter = treeFmt.config.build.wrapper;

      checks.formatting = treeFmt.config.build.check self;
    };
}
