{
  diskoConfig = import ../../disko.nix;

  newConfig = {
    disko.devices.zpool."zroot".datasets = {
      "ds1" = {
        options = {
          recordsize = "8k";
        };
      };
    };
  };

  extraTestScript = ''
    assert_zfs_property("zroot/ds1/persist", "recordsize", "8K")
  '';
}
