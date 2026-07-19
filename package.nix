{
  lib,
  fetchFromGitHub,
  rustPlatform,
}:

rustPlatform.buildRustPackage (finalAttrs: {
  pname = "disko-zfs";
  inherit ((lib.importTOML ./Cargo.toml).package) version;

  src = lib.fileset.toSource {
    root = ./.;
    fileset = lib.fileset.unions [
      ./src
      ./tests
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
