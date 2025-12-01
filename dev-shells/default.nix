{
  perSystem = {pkgs, ...}: {
    devShells.default = pkgs.mkShell {
      packages = with pkgs; [
        rustc
        rust-analyzer
        cargo
        rustfmt
      ];
    };
  };
}
