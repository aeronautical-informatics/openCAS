{
  inputs = {
    utils.url = "github:numtide/flake-utils";
    utils.inputs.nixpkgs.follows = "nixpkgs";
    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";
    naersk.url = "github:nmattia/naersk";
    naersk.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, utils, fenix, naersk }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages."${system}";
        toolchain = with fenix.packages.${system};
          combine [
            stable.rustc
            stable.cargo
            stable.clippy
            stable.rustfmt
            targets."wasm32-unknown-unknown".stable.rust-std
          ];
        naersk-lib = (naersk.lib.${system}.override {
          cargo = toolchain;
          rustc = toolchain;
        });
      in
      rec {
        # nix build
        packages.advisory_viewer = naersk-lib.buildPackage {
          root = ./.;
          doCheck = true;
          nativeBuildInputs = with pkgs; [ makeWrapper pkg-config ];
          buildInputs = with pkgs; [
            libxkbcommon
            openssl
            xorg.libX11
            xorg.libXcursor
            xorg.libXi
            xorg.libXrandr
            xorg.libxcb
          ];
          overrideMain = (_: {
            postFixup = ''
              wrapProgram $out/bin/advisory_viewer \
                --prefix LD_LIBRARY_PATH ":" ${pkgs.libGL}/lib 
            '';
          });
        };
        defaultPackage = packages.advisory_viewer;

        # nix run
        apps.advisory_viewer = utils.lib.mkApp { drv = packages.advisory_viewer; };
        defautltApp = apps.advisory_viewer;

        # nix develop
        devShell = pkgs.mkShell {
          nativeBuildInputs = with pkgs; packages.advisory_viewer.nativeBuildInputs ++ [
            binaryen
            httplz
            toolchain
            cargo-flamegraph
            wasm-bindgen-cli
          ];
          buildInputs = packages.advisory_viewer.buildInputs;
          shellHook = ''
            export LD_LIBRARY_PATH="${pkgs.libGL}/lib"
            export IS_NIX_BUILD=true
          '';
        };
      });
}
