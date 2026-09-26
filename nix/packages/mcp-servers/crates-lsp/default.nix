{ fetchFromGitHub
, lib
, rustPlatform
, ...
}:
rustPlatform.buildRustPackage (finalAttrs: {
  pname = "crates-lsp";
  version = "0.4.2";

  src = fetchFromGitHub {
    owner = "MathiasPius";
    repo = "crates-lsp";
    tag = "v${finalAttrs.version}";
    hash = "sha256-s42nWQC2tD7vhQNPdTQNRokwXqeBhELidVYTlos+No0=";
  };

  cargoHash = "sha256-XqUWcbaOZXRWzIvL9Kbo6Unl0rmeGxHO4+674uHukAs=";

  checkFlags = [
    # These tests fail on system `x86_64-darwin` with error:
    # failed to create crates-lsp .gitignore file.: Os { code: 13, kind: PermissionDenied, message: "Permission denied" }
    "--skip=crates::api::tests::get_common_crates"
    "--skip=crates::sparse::tests::get_common_crates"
  ];
})
