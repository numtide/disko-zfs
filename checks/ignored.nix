{ lib, ... }:
{
  diskoConfig = lib.recursiveUpdate (import ../disko.nix) {
    disko.devices.zpool."zroot".datasets."ds1/persist".options.":test-remove" = "letsgo";
  };

  diskoZfs = {
    ignored.datasets = [ "zroot/ds1/persist/postgresql" ];
    ignored.properties = [
      ":test-add"
      ":test-remove"
    ];

    datasets = {
      "zroot/ds1/persist" = {
        properties = lib.mkForce {
          ":test-add" = "letsgo";
          mountpoint = "legacy";
        };
      };
      "zroot/ds1/persist/postgresql" = { };
    };
  };

  extraTestScript = ''
    assert_zfs_dataset_not_exists("zroot/ds1/persist/postgresql")
    assert_zfs_property("zroot/ds1/persist", ":test-add", "-")
    assert_zfs_property("zroot/ds1/persist", ":test-remove", "letsgo")
  '';
}
