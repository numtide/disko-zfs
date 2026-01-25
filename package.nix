{
  lib,
  fetchFromGitHub,
  rustPlatform,
}:

rustPlatform.buildRustPackage (finalAttrs: {
  pname = "disko-zfs";
  version = "unknown";

  src = lib.fileset.toSource {
    root = ./.;
    fileset = lib.fileset.unions [
      ./src
      ./Cargo.toml
      ./Cargo.lock
    ];
  };

  buildType = "debug";

  cargoLock.lockFile = ./Cargo.lock;

  meta = {
    description = "Declarative ZFS dataset management.";
    homepage = "https://github.com/numtide/disko-zfs";
    license = lib.licenses.gpl3;
    maintainers = [ ];
    mainProgram = "disko-zfs";
  };
})
