{
  diskoConfig = import ../disko.nix;

  diskoZfs = {
    datasets = {
      "zroot/ds1/persist/postgresql" = {
        properties = {
          "recordsize" = "8k";
        };
      };
      "zroot/ds1/root" = {
        properties = {
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
