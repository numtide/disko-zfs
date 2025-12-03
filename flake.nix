{
  description = "Description for the project";

  inputs = {
    flake-parts.url = "github:hercules-ci/flake-parts";
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    disko.url = "github:nix-community/disko";
  };

  outputs =
    inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } (
      let
        diskoDevices = {
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
      in
      { lib, ... }:
      {
        imports = [
          ./dev-shells/default.nix
        ];

        perSystem =
          { pkgs, config, ... }:
          {
            packages.disko-zfs = pkgs.callPackage ./package.nix { };
            packages.default = config.packages.disko-zfs;

            checks.basic =
              let
                diskoLib = import "${inputs.disko}/lib" {
                  inherit (pkgs) lib;
                  makeTest = import "${inputs.nixpkgs}/nixos/tests/make-test-python.nix";
                  eval-config = import "${inputs.nixpkgs}/nixos/lib/eval-config.nix";
                  qemu-common = import "${inputs.nixpkgs}/nixos/lib/qemu-common.nix";
                };
              in
              diskoLib.testLib.makeDiskoTest {
                inherit pkgs;
                name = "basic";

                disko-config = {
                  disko.devices = diskoDevices;
                };

                extraInstallerConfig =
                  { ... }:
                  {
                    networking.hostId = "deadbeef";
                    boot.kernelPackages = pkgs.linuxKernel.packages.linux_6_12;
                  };

                extraSystemConfig =
                  { config, pkgs, ... }:
                  {
                    imports = [
                      inputs.self.nixosModules.default
                    ];

                    disko.zfs.enable = true;

                    # virtualisation.directBoot.enable = false;
                    # virtualisation.mountHostNixStore = false;
                    # virtualisation.useEFIBoot = true;
                    # virtualisation.installBootLoader = true;
                    # virtualisation.useDefaultFilesystems = false;
                    # virtualisation.fileSystems = lib.mkForce { };

                    # # config for tests to make them run faster or work at all
                    # documentation.enable = false;
                    # hardware.enableAllFirmware = lib.mkForce false;

                    # boot.zfs.devNodes = "/dev/disk/by-uuid"; # needed because /dev/disk/by-id is empty in qemu-vms

                    # boot.loader.systemd-boot.enable = true;
                    # boot.loader.timeout = 0;
                    # boot.loader.efi.canTouchEfiVariables = true;
                    networking.hostId = "deadbeef";
                    boot.kernelPackages = pkgs.linuxKernel.packages.linux_6_12;
                    boot.supportedFilesystems = [ "zfs" ];
                    boot.initrd.systemd.enable = true;
                  };

                extraTestScript = ''
                  machine.succeed("systemctl status zfs-disko.service")
                  machine.wait_for_unit("multi-user.target");
                '';

                # testScript =
                #   { nodes, ... }:
                #   ''
                #     import shlex
                #     import shutil
                #     import tempfile
                #     import time

                #     tmp_disk_image = tempfile.NamedTemporaryFile()

                #     def create_test_machine(
                #         disk, **kwargs
                #     ):  # taken from <disko/master/lib/tests.nix>
                #         # Use qemu-common from nixpkgs to get the proper QEMU binary with correct machine type and flags
                #         # shlex.split properly handles the command string with options like "-machine virt,gic-version=max"
                #         start_command = shlex.split("${qemuBinaryString}") + [
                #             "-m",
                #             "1024",
                #             '-drive',
                #             f"file={disk},id=drive1,if=none,index=1,werror=report,format=${nodes.machine.disko.imageBuilder.imageFormat}",
                #             '-device',
                #             "virtio-blk-pci,drive=drive1"
                #         ]
                #         ${lib.optionalString true ''
                #           start_command += ["-drive",
                #             "if=pflash,format=raw,unit=0,readonly=on,file=${pkgs.OVMF.firmware}",
                #             "-drive",
                #             "if=pflash,format=raw,unit=1,readonly=on,file=${pkgs.OVMF.variables}"
                #           ]
                #         ''}
                #         machine = create_machine(start_command=" ".join(start_command), **kwargs)
                #         driver.machines.append(machine)
                #         return machine

                #     shutil.copyfile(
                #       "${nodes.machine.system.build.diskoImages}/${lib.escapeShellArg nodes.machine.disko.devices.disk.x.imageName}.${nodes.machine.disko.imageBuilder.imageFormat}",
                #       tmp_disk_image.name,
                #     )

                #     machine = create_test_machine(disk=tmp_disk_image.name, name="booted_machine")

                #     machine.start()
                #     time.sleep(20)
                #     print(machine.succeed("systemctl"))
                #     ${lib.concatMapStringsSep "\n"
                #       (unit: ''
                #         print(machine.succeed("systemctl status ${unit}"))
                #       '')
                #       [
                #         "getty@tty1.service"
                #         "backdoor.service"
                #         "dhcpcd.service"
                #         "getty.target"
                #         "linger-users.service"
                #         "network-setup.service"
                #         "nscd.service"
                #         "reload-systemd-vconsole-setup.service"
                #         "remote-fs.target"
                #         "resolvconf.service"
                #         "systemd-ask-password-wall.path"
                #         "systemd-logind.service"
                #         "systemd-modules-load.service"
                #         "systemd-oomd.service"
                #         "systemd-sysctl.service"
                #         "systemd-user-sessions.service"
                #         "zfs.target"
                #       ]
                #     }
                #     machine.wait_for_unit("multi-user.target")
                #   '';
              };
          };

        flake.nixosModules.default = lib.modules.importApply ./nixos/modules/default.nix {
          inherit inputs;
        };
        flake.overlays.default = final: _: {
          disko-zfs = final.callPackage ./package.nix { };
        };

        flake.nixosConfigurations.default = inputs.nixpkgs.lib.nixosSystem {
          system = "x86_64-linux";
          modules = [
            (
              { config, pkgs, ... }:
              {
                imports = [
                  inputs.self.nixosModules.default
                  inputs.disko.nixosModules.default
                ];

                disko = {
                  checkScripts = true;
                  devices = diskoDevices;
                };

                # virtualisation.directBoot.enable = false;
                # virtualisation.mountHostNixStore = false;
                # virtualisation.useEFIBoot = true;

                # config for tests to make them run faster or work at all
                documentation.enable = false;
                hardware.enableAllFirmware = lib.mkForce false;

                boot.loader.systemd-boot.enable = true;
                boot.loader.timeout = 0;
                boot.loader.efi.canTouchEfiVariables = true;
                networking.hostId = "deadbeef";
                boot.kernelPackages = pkgs.linuxKernel.packages.linux_6_12;
                boot.supportedFilesystems = [ "zfs" ];
                boot.initrd.systemd.enable = true;
              }
            )
          ];
        };

        systems = [
          "x86_64-linux"
          "aarch64-linux"
          "aarch64-darwin"
          "x86_64-darwin"
        ];
      }
    );
}
