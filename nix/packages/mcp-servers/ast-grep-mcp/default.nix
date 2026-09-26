{ lib
, pkgs
, inputs
, ...
}:

let
  src = pkgs.fetchFromGitHub {
    owner = "ast-grep";
    repo = "ast-grep-mcp";
    rev = "674272f1adb56fd1fe48a546952c7ffbe72c09e6";
    hash = "sha256-BdYvsi5o6AkCD2XCFWDfY/zVL58FcMtznpcINmqCvoo=";
  };

  workspace = inputs.uv2nix.lib.workspace.loadWorkspace {
    workspaceRoot = src;
  };

  overlay = workspace.mkPyprojectOverlay {
    sourcePreference = "wheel";
  };

  pyprojectOverrides = _final: _prev: { };

  python = pkgs.python313;

  pythonSet =
    (pkgs.callPackage inputs.pyproject-nix.build.packages {
      inherit python;
    }).overrideScope
      (
        lib.composeManyExtensions [
          inputs.pyproject-build-systems.overlays.default
          overlay
          pyprojectOverrides
        ]
      );

  venv = pythonSet.mkVirtualEnv "ast-grep-mcp" workspace.deps.default;

  # The ast-grep-server MCP binary shells out to the `ast-grep` CLI
  # for its actual matching work — the venv only ships the server
  # itself, so without prefixing `ast-grep` onto PATH at wrap time
  # every find_code call fails with "Command 'ast-grep' not found".
  package =
    pkgs.runCommand "ast-grep-mcp"
      {
        inherit (venv) meta;
        nativeBuildInputs = [ pkgs.makeWrapper ];
      }
      ''
        mkdir -p $out/bin
        makeWrapper ${venv}/bin/ast-grep-server $out/bin/ast-grep-server \
          --prefix PATH : ${lib.makeBinPath [ pkgs.ast-grep ]}
      '';
in
package
  // {
  meta = with lib; {
    description = "MCP Server using ast-grep for semantic code search and transformation";
    homepage = "https://github.com/ast-grep/ast-grep-mcp";
    license = licenses.mit;
    maintainers = [ ];
  };
}
