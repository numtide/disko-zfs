{ lib, ... }:
{
  diskoConfig = import ../../disko.nix;

  initialConfig = {
    disko.devices.zpool."zroot".datasets."ds1/persist".options.":test-remove" = "letsgo";
  };

  newConfig = {
    disko.zfs.settings = {
      ignoredDatasets = [ "zroot/ds1/persist/postgresql" ];
      ignoredProperties = [
        ":test-add"
        ":test-remove"
      ];
    };
    disko.devices.zpool."zroot" = {
      datasets = {
        "ds1/persist" = {
          options = lib.mkForce {
            ":test-add" = "letsgo";
            mountpoint = "legacy";
          };
        };
        "ds1/persist/postgresql" = {
          type = "zfs_fs";
          options.mountpoint = "/var/lib/postgresql";
        };
      };
    };
  };

  extraTestScript = ''
    assert_zfs_dataset_not_exists("zroot/ds1/persist/postgresql")
    assert_zfs_property("zroot/ds1/persist", ":test-add", "-")
    assert_zfs_property("zroot/ds1/persist", ":test-remove", "letsgo")
  '';
}
