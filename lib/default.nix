{ inputs, lib }:
let
  diskoLib = import "${inputs.disko}/lib" {
    inherit lib;
    makeTest = import "${inputs.nixpkgs}/nixos/tests/make-test-python.nix";
    eval-config = import "${inputs.nixpkgs}/nixos/lib/eval-config.nix";
    qemu-common = import "${inputs.nixpkgs}/nixos/lib/qemu-common.nix";
  };
in
{
  mkDiskoZfsTest =
    {
      name,
      pkgs,
      diskoConfig ? { },
      initialConfig ? { },
      newConfig ? { },
      extraTestScript ? "",
    }:
    diskoLib.testLib.makeDiskoTest {
      inherit pkgs name;

      disko-config = diskoConfig;

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
            initialConfig
            (lib.optionalAttrs (initialConfig.disko or { } ? devices) {
              # https://github.com/nix-community/disko/blob/master/lib/tests.nix#L168
              disko.devices = lib.mkForce initialConfig.disko.devices;
            })
          ];

          disko.zfs = {
            enable = true;
            settings = lib.mkMerge [
              { logLevel = "trace"; }
            ];
          };

          networking.hostId = "deadbeef";
          boot.kernelPackages = pkgs.linuxKernel.packages.linux_6_12;
          boot.supportedFilesystems = [ "zfs" ];
          boot.initrd.systemd.enable = true;

          specialisation."new".configuration = lib.mkMerge [
            newConfig
            (lib.optionalAttrs (newConfig.disko or { } ? devices) {
              # https://github.com/nix-community/disko/blob/master/lib/tests.nix#L168
              disko.devices = lib.mkForce newConfig.disko.devices;
            })
          ];
        };

      extraTestScript = ''
        machine.wait_for_unit("multi-user.target");
        machine.succeed("systemctl status disko-zfs.service")

        def zfs_get_properties(dataset, properties):
          output = {}
          for property in properties:
            output[property] = machine.succeed(f"zfs get {property} {dataset} -o value -H").strip()
          return output

        def zfs_get_property(dataset, property):
          return zfs_get_properties(dataset, [property])[property]

        def assert_zfs_property(dataset, property, value):
          actual_value = zfs_get_property(dataset, property)
          assert actual_value == value, f"{dataset}: {actual_value} != {value}"

        def assert_zfs_dataset_exists(dataset):
          (status, _stdout) = machine.execute(f"zfs list {dataset}")
          assert status == 0, f"{dataset} doesn't exist"

        def assert_zfs_dataset_not_exists(dataset):
          (status, _stdout) = machine.execute(f"zfs list {dataset}")
          assert status != 0, f"{dataset} does exist"

        machine.succeed("/run/current-system/specialisation/new/bin/switch-to-configuration test")
        machine.succeed("systemctl restart disko-zfs")

        ${extraTestScript}
      '';
    };
}
