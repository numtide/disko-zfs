{ lib, ... }:

lib.modules.importApply ./nixos/modules {
  overlay = import ./overlay.nix;
}
