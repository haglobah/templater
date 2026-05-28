{
  description = "A project by ?.";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
  };

  outputs = inputs @ {
    flake-parts,
    self,
    ...
  }:
    flake-parts.lib.mkFlake {inherit inputs;} {
      imports = [];
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
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            nixfmt
            typescript-language-server #if typescript
            just             #if just
            typst            #if typst
            racket           #if (or racket pollen)
            nodejs_24        #if (or node typescript cljs astro slidev)
            bun              #if bun
            zulu             #if (or clj java cljs)
            clojure          #if (or clj cljs)
            clojure-lsp      #if (or clj cljs)
            #if haskell
            haskell.compiler."ghc98"
            haskell.packages."ghc98".haskell-language-server
            cabal-install
            #endif haskell
            #if rust
            cargo
            #endif rust
            #if tauri
            ##nativeBuildInputs
            pkg-config
            gobject-introspection
            cargo
            bun
            ##buildInputs
            at-spi2-atk
            atkmm
            cairo
            gdk-pixbuf
            glib
            gtk3
            harfbuzz
            librsvg
            libsoup_3
            pango
            webkitgtk_4_1
            openssl
            #endif tauri
            #if python
            (python3.withPackages (pp: [
              pp.requests # for example
            ]))
            #endif python
            #if gleam
            gleam
            erlang
            rebar3
            inotify-tools    #if lustre
            #endif gleam
            elixir_1_18      #if (or elixir)
          ];
          shellHook = ''
          '';
        };
      };
      flake = {
        # The usual flake attributes can be defined here, including system-
        # agnostic ones like nixosModule and system-enumerating ones, although
        # those are more easily expressed in perSystem.
      };
    };
}
