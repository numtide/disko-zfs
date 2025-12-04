{
  disko = {
    devices = {

      disk = {
        x = {
          imageSize = "4G";
          type = "disk";
          device = "/dev/vda";
          content = {
            type = "gpt";
            partitions = {
              ESP = {
                size = "64M";
                type = "EF00";
                content = {
                  type = "filesystem";
                  format = "vfat";
                  mountpoint = "/boot";
                  mountOptions = [ "umask=0077" ];
                };
              };
              zfs = {
                size = "100%";
                content = {
                  type = "zfs";
                  pool = "zroot";
                };
              };
            };
          };
        };
      };

      zpool."zroot" = {
        type = "zpool";

        options = {
          ashift = "12";
        };

        rootFsOptions = {
          xattr = "sa";
          recordsize = "128K";
          compression = "zstd-2";
          atime = "off";
          dnodesize = "auto";
          mountpoint = "none";
        };

        datasets = {
          "ds1" = {
            type = "zfs_fs";
            options.mountpoint = "none";
            mountpoint = null;
          };

          "ds1/root" = {
            type = "zfs_fs";
            options.mountpoint = "legacy";
            mountpoint = "/";
          };

          "ds1/nix" = {
            type = "zfs_fs";
            options.mountpoint = "legacy";
            mountpoint = "/nix";
          };

          "ds1/persist" = {
            type = "zfs_fs";
            options.mountpoint = "legacy";
            mountpoint = "/nix/persist";
          };
        };
      };
    };
  };
}
