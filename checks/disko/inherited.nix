{
  diskoConfig = import ../../disko.nix;

  diskoZfs = {
    datasets = {
      "zroot/ds1" = {
        properties = {
          recordsize = "8k";
        };
      };
    };
  };

  extraTestScript = ''
    assert_zfs_property("zroot/ds1/persist", "recordsize", "8K")
  '';
}
