{ lib
, rustPlatform
, ...
}:
# samply-mcp — stdio MCP server that answers structured queries about
# samply Firefox Profiler JSON traces. Use to inspect profiles produced
# by `samply record …` runs against benchmarks / examples without
# manually grepping the gzipped JSON.
#
# Source is VENDORED under ./source (Cargo.toml, Cargo.lock, src/). It is
# vendored (rather than referencing a developer-local path via
# `fetchGit`/`file://`) so the dev shell + every flake check builds
# reproducibly anywhere, CI included. To update, re-copy the src files +
# Cargo.{toml,lock} from the upstream samply-mcp source. Deps are all
# crates.io, so `cargoLock.lockFile` vendors them via the normal
# registry.
rustPlatform.buildRustPackage {
  pname = "samply-mcp";
  version = "0.1.0";

  src = ./source;

  cargoLock.lockFile = ./source/Cargo.lock;

  # Tests would need profile fixtures.
  doCheck = false;

  meta = {
    description = "MCP server for analyzing samply Firefox Profiler JSON profiles";
    license = lib.licenses.eupl12;
    mainProgram = "samply-mcp";
  };
}
