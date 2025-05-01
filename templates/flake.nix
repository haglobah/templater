{
  description = "A project by ?.";

  inputs = {
    nixpkgs.url = "https://flakehub.com/f/DeterminateSystems/nixpkgs-weekly/*.tar.gz";
    devshell.url = "github:numtide/devshell"; #if devshell
    devshell.inputs.nixpkgs.follows = "nixpkgs"; #if devshell
  };

  outputs = inputs @ {
    flake-parts,
    self,
    ...
  }:
    flake-parts.lib.mkFlake {inherit inputs;} {
      imports = [
        inputs.devshell.flakeModule #if devshell
      ];
      systems = ["x86_64-linux" "aarch64-linux" "aarch64-darwin" "x86_64-darwin"];
      perSystem = {
        config,
        self',
        inputs',
        pkgs,
        system,
        ...
      }: {
        _module.args.pkgs = import self.inputs.nixpkgs {
          inherit system;
          config.allowUnfree = true;
        };
        # Per-system attributes can be defined here. The self' and inputs'
        # module parameters provide easy access to attributes of the same
        # system.

        # Equivalent to  inputs'.nixpkgs.legacyPackages.hello;
        packages.default = pkgs.hello;
        #if devshell
        devshells.default = {
          env = [
            # { name = "MY_ENV_VAR"; value = "SOTRUE"; }
          ];
          packages = [
            pkgs.racket     #if (or racket pollen)
            pkgs.nodejs_22  #if (or node cljs astro)
            pkgs.zulu       #if (or clj java cljs)
          ];
          commands = [
            #if cljs
            {
              name = "create";
              command = "npx create-cljs-project $1";
              help = "Create a new cljs app";
            }
            {
              name = "watch";
              command = "npx shadow-cljs watch $1";
              help = "Run cljs dev server";
            }
            {
              name = "compile";
              command = "npx shadow-cljs compile $1";
              help = "Build a release";
            }
            #endif
          ];
        };
        #endif devshell
      };
      flake = {
        # The usual flake attributes can be defined here, including system-
        # agnostic ones like nixosModule and system-enumerating ones, although
        # those are more easily expressed in perSystem.
      };
    };
}
#if devshell
devshells.default = {
  packages = [
    pkgs.gleam
    pkgs.nodejs #if node
  ];
};
#endif

Some setup for Clojure with devshell #if (and clj lala devshell)
Some setup for devshell #if (or la le)

einfach so hier.
