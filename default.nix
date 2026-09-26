{ system ? builtins.currentSystem
, packageName ? "default"
,
}:
let
  flake = (import ./nix/flake-compat.nix { inherit system; });
in
flake.inputs.self.packages.${ system }.${ packageName }
