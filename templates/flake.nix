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
          packages = with pkgs; [
            nixfmt-rfc-style
            #if just
            just
            concurrently
            #endif just
            racket           #if (or racket pollen)
            nodejs_22        #if (or node cljs astro slidev)
            zulu             #if (or clj java cljs)
            clojure          #if clj
            clojure-lsp      #if clj
            #if haskell
            haskell.compiler."ghc98"
            haskell.packages."ghc98".haskell-language-server
            cabal-install
            #endif haskell
            #if gleam
            gleam
            erlang
            rebar3
            inotify-tools    #if lustre
            #endif gleam
            elixir_1_18      #if (or elixir)
          ];
          commands = [
            {
              name = "run";
              command = "echo \"Run me!\"";
              help = "Launch the application";
            }
            #if haskell
            { name = "cr"; command = "cabal run "; help = "Alias for 'cabal run'"; }
            { name = "cu"; command = "cabal update"; help = "'cabal update'"; }
            #endif haskell
            { name = "ie"; command = "iex -S mix"; help = "Run iex with the application loaded"; } #if elixir
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
            #endif cljs
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
