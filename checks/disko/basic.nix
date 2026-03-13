{
  diskoConfig = import ../../disko.nix;

  newConfig = {
    disko.devices.zpool."zroot".datasets = {
      "ds1/persist/postgresql" = {
        type = "zfs_fs";
        options = {
          "mountpoint" = "legacy";
          "recordsize" = "8k";
        };
        mountpoint = "/var/lib/postgresql";
      };
      "ds1/root" = {
        options = {
          ":disko-zfs" = "activated";
        };
      };
    };
  };

  extraTestScript = ''
    assert_zfs_property("zroot/ds1/persist/postgresql", "recordsize", "8K")
    assert_zfs_property("zroot/ds1/root", ":disko-zfs", "activated")
  '';
}
