# `disko-zfs`

> Manage your ZFS datasets declaratively

## Getting Started

First you have add this flake as a flake input:

```nix
inputs = {
  # other inputs ...

  disko-zfs = {
    url = "github:numtide/disko-zfs";
    inputs.nixpkgs.follows = "nixpkgs";
    inputs.flake-parts.follows = "flake-parts";
    inputs.disko.follows = "disko";
  };
};
```

Next you need to add the `diskoZfs` module to your NixOS configuration, how spefically you need to do that is highly dependant, but in the most basic case something like the following should suffice:

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs?ref=nixos-unstable";

    disko-zfs.url = "github:numtide/disko-zfs";
  };

  outputs = { nixpkgs, disko-zfs, ...}:
  {
    nixosConfigurations.default = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        {
          imports = [
            inputs.disko-zfs.nixosModules.default
          ];

          # your configuration here
        }
      ];
    };
  };
}
```

With the module in-place, you can use `disko-zfs` like so:

```nix
{
  disko.zfs = {
    enable = true;

    settings = {
      datasets = {
        "zroot/ds1/persist/var/lib/postgresql" = {
          properties."recordsize" = "128K";
        };
        "zroot/ds1/nix" = { };
        "zroot/ds1/home" = { };
        "zroot/ds1/root" = { };
      };
    };
  };
}
```

Now, when you switch your machine to this configuration, `disko-zfs` will ensure that the declared datasets along with the declared properties are configured. Any datasets not declared will be left untouched, however `disko-zfs` will list the commands it would run to destroy any extra datasets.

> [!NOTE]
> `disko-zfs` will NEVER remove any datasets. It will however unset properties, which if they hold unique information could lead to dataloss, you have been warned!

## Disko + `disko-zfs`

If you also utilize [Disko](https://github.com/nix-community/disko) in your NixOS configuration, `disko-zfs` will detect this and automatically include ZFS pools and datasets declared through Disko in its own configuration. AS such if you have Disko, you should prefer Disko's way of declaring ZFS datasets and pools. See the [ZFS Disko example](https://github.com/nix-community/disko/blob/master/example/zfs.nix) to get an idea of how to do this.

## Dry Running `disko-zfs`

As a specialty `disko-zfs` adds a activation script which only executes during dry activation, which will print out the command `disko-zfs` would run if you were to switch to that NixOS configuration. As such if you run:

```
nixos-rebuild --flake .#default --sudo dry-activate
```

`disko-zfs` will let you know what it would do.

---

This project is supported by [Numtide](https://numtide.com/).

![Untitledpng](https://codahosted.io/docs/6FCIMTRM0p/blobs/bl-sgSunaXYWX/077f3f9d7d76d6a228a937afa0658292584dedb5b852a8ca370b6c61dabb7872b7f617e603f1793928dc5410c74b3e77af21a89e435fa71a681a868d21fd1f599dd10a647dd855e14043979f1df7956f67c3260c0442e24b34662307204b83ea34de929d)

We are a team of independent freelancers that love open source.  We help our
customers make their project lifecycles more efficient by:

- Providing and supporting useful tools such as this one
- Building and deploying infrastructure, and offering dedicated DevOps support
- Building their in-house Nix skills, and integrating Nix with their workflows
- Developing additional features and tools
- Carrying out custom research and development.

[Contact us](https://numtide.com/contact) if you have a project in mind, or if
you need help with any of our supported tools, including this one. We'd love to
hear from you.
