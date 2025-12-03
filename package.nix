{
  lib,
  fetchFromGitHub,
  rustPlatform,
}:

rustPlatform.buildRustPackage (finalAttrs: {
  pname = "disko-zfs";
  version = "unknown";

  src = ./.;

  buildType = "debug";

  cargoHash = "sha256-aqq4k92U9aKlZa8byMFfJUvr1EQ0MJExy30vdzvd5nI=";

  meta = {
    description = "Declarative ZFS dataset management.";
    homepage = "https://git.numtide.com/magic_rb/disko-zfs";
    license = lib.licenses.gpl3;
    maintainers = [ ];
    mainProgram = "disko-zfs";
  };
})
