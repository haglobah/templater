{
  description = "A conditional templating tool";

  inputs = {
    nixpkgs.url = "https://flakehub.com/f/DeterminateSystems/nixpkgs-weekly/*.tar.gz";
    flake-parts.url = "github:hercules-ci/flake-parts";
  };

  outputs = inputs @ { self, flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [];
      systems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" "x86_64-darwin" ];
      perSystem = { config, self', inputs', pkgs, system, ... }: {
        packages.rust = pkgs.rustPlatform.buildRustPackage rec {
          pname = "templater";
          meta.mainProgram = "templater";
          version = "0.2";
          cargoLock.lockFile = ./Cargo.lock;
          src = ./.;
        };

        checks.rust = self'.packages.rust;

        packages.default = pkgs.writeShellApplication {
          name = "templater";
          runtimeInputs = [ self'.packages.rust ];
          text = ''
          ${self'.packages.rust}/bin/templater --from ${self}/templates "$@"
        '';
        };

        apps.default = {
          type = "app";
          program = "${pkgs.lib.getExe self'.packages.default}";
        };

        devShells.default = pkgs.mkShell {
          buildInputs = [
            pkgs.rustc
            pkgs.cargo
            pkgs.rust-analyzer
            pkgs.rustfmt
          ];
        };
      };
    };
}
