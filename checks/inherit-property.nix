{ lib, ... }:
{
  diskoConfig = lib.recursiveUpdate (import ../disko.nix) {
    disko.devices.zpool."zroot".datasets."ds1/persist".options.":test" = "letsgo";
  };

  diskoZfs = {
    datasets = {
      "zroot/ds1/persist" = {
        properties = lib.mkForce {
          mountpoint = "legacy";
        };
      };
    };
  };

  extraTestScript = ''
    assert_zfs_property("zroot/ds1/persist", ":test", "-")
  '';
}
