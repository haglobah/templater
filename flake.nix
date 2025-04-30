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
          mypython = pkgs.python3;
        in
        {
        packages.default = pkgs.writeShellApplication {
          name = "templater";
          runtimeInputs = [ mypython ];
          text = ''
            exec ${mypython}/bin/python ${self}/templater.py "$@"
          '';
        };

        apps.default = {
          type = "app";
          program = "${pkgs.lib.getExe self'.packages.default}";
        };

        devShells.default = pkgs.mkShell {
          buildInputs = [ mypython ];
        };
      };
    };
}
