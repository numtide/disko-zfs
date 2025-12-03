{
  perSystem = {pkgs, ...}: {
    devShells.default = pkgs.mkShell {
      packages = with pkgs; [
        nixfmt-rfc-style
        rustc
        rust-analyzer
        cargo
        rustfmt
        (writeShellApplication {
          name = "get_binary";
          runtimeInputs = [
            jq
          ];
          text = ''
            TARGET_DIR="$(cargo metadata --format-version 1 --no-deps | jq -r '.target_directory')"
            BIN_NAME="$(cargo metadata --format-version 1 --no-deps | jq -r '.packages[].targets[] | select( .kind | map(. == "bin") | any ) | .name')"
            BINARY_PATH="$TARGET_DIR/debug/$BIN_NAME"

            echo "$BINARY_PATH"
          '';
        })
      ];
    };
  };
}
