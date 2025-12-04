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
      diskoConfig,
      diskoZfs ? { },
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
          ];

          disko.zfs = {
            enable = true;
            settings = diskoZfs;
          };

          networking.hostId = "deadbeef";
          boot.kernelPackages = pkgs.linuxKernel.packages.linux_6_12;
          boot.supportedFilesystems = [ "zfs" ];
          boot.initrd.systemd.enable = true;
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

        ${extraTestScript}
      '';
    };
}
