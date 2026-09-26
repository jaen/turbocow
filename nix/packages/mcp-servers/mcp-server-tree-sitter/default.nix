{ lib
, pkgs
, inputs
, ...
}:
let
  src = pkgs.fetchFromGitHub {
    owner = "wrale";
    repo = "mcp-server-tree-sitter";
    rev = "v0.5.1"; # You can update this to latest version
    hash = "sha256-YRnTEzs8OAY0ADkTT3b20owVDnEJ5om4VoDFDRbjXVs=";
  };

  workspace = inputs.uv2nix.lib.workspace.loadWorkspace {
    workspaceRoot = src;
  };

  overlay = workspace.mkPyprojectOverlay {
    sourcePreference = "wheel";
  };

  pyprojectOverrides = _final: _prev: { };

  python = pkgs.python311;

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

  venv = pythonSet.mkVirtualEnv "mcp-server-tree-sitter-env" workspace.deps.default;

  # Create a wrapper that only exposes the mcp-server-tree-sitter binary
  package =
    pkgs.runCommand "mcp-server-tree-sitter"
      {
        inherit (venv) meta;
      }
      ''
        mkdir -p $out/bin

        # Only link the mcp-server-tree-sitter binary
        ln -s ${venv}/bin/mcp-server-tree-sitter $out/bin/mcp-server-tree-sitter
      '';
in
package
  // {
  meta = with lib; {
    description = "MCP Server for Tree-sitter code analysis";
    homepage = "https://github.com/wrale/mcp-server-tree-sitter";
    license = licenses.mit;
    maintainers = [ ];
  };
}
