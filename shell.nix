{ system ? builtins.currentSystem
, shellName ? "default"
,
}:
let
  flake = (import ./nix/flake-compat.nix { inherit system; });
in
flake.inputs.self.devShells.${ system }.${ shellName }
