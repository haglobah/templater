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
      perSystem = { config, self', inputs', pkgs, system, ... }:
        let
          mypython = pkgs.python3.withPackages (p: [
            p.colorama
          ]);
        in
          {
            packages.rust = pkgs.rustPlatform.buildRustPackage rec {
              pname = "templater";
              meta.mainProgram = "templater";
              version = "0.2";
              cargoLock.lockFile = ./Cargo.lock;
              src = ./.;
            };

            packages.default = pkgs.writeShellApplication {
              name = "templater";
              runtimeInputs = [ self'.packages.rust ];
              text = ''
              ${self'.packages.rust}/bin/templater --from ${self}/templates "$@"
            '';
            };

            packages.python = pkgs.writeShellApplication {
              name = "templater";
              runtimeInputs = [ mypython ];
              text = ''
                exec ${mypython}/bin/python ${self}/templater.py --from ${self}/templates "$@"
              '';
            };

            apps.default = {
              type = "app";
              program = "${pkgs.lib.getExe self'.packages.default}";
            };

            devShells.default = pkgs.mkShell {
              buildInputs = [
                mypython
                pkgs.rustc
                pkgs.cargo
                pkgs.rust-analyzer
                pkgs.rustfmt
              ];
            };

            checks.python-templater-tests = pkgs.runCommand "templater-tests" {
              buildInputs = [ mypython ];
              # Pass the test + script files into the build environment
              src = self;
            } ''
                cp $src/test_templater.py .
                cp $src/templater.py .
                ${mypython}/bin/python test_templater.py
                touch $out
              '';
          };
    };
}
