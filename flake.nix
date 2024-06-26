{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-24.05";
    utils.url = "github:numtide/flake-utils";
    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";
    naersk.url = "github:nix-community/naersk";
    naersk.inputs.nixpkgs.follows = "nixpkgs";
    treefmt-nix.url = "github:numtide/treefmt-nix";
    treefmt-nix.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, utils, fenix, naersk, treefmt-nix }:
    utils.lib.eachDefaultSystem (system:
      let
        lib = nixpkgs.lib;

        pkgs = nixpkgs.legacyPackages."${system}";

        treefmt' = {
          projectRootFile = "flake.nix";
          settings = with builtins; (fromTOML (readFile ./treefmt.toml));
        };

        toolchain = with fenix.packages.${system};
          combine [
            stable.rustc
            stable.cargo
            stable.clippy
            stable.rustfmt
            targets.wasm32-unknown-unknown.stable.rust-std
          ];

        naersk-lib = (naersk.lib.${system}.override {
          cargo = toolchain;
          rustc = toolchain;
        });

        requiredLibs = with pkgs;
          [
            libGL
            wayland
            xorg.libX11
            xorg.libXcursor
            xorg.libXi
            xorg.libXrandr
            xorg.libXxf86vm
            xorg.libxcb
          ] ++ lib.optionals stdenv.isLinux [ libxkbcommon wayland ];

        libPath = lib.makeLibraryPath requiredLibs;
      in
      rec {
        # nix build
        packages.advisory_viewer = naersk-lib.buildPackage rec {
          name = "advisory_viewer";
          src = ./.;
          doCheck = true;
          cargoBuildOptions = x: x ++ [ "-p" name ];
          cargoTestOptions = x: x ++ [ "-p" name ];
          # nativeBuildInputs = with pkgs; [ pkg-config ];
          overrideMain = (_: {
            postFixup = ''
              patchelf --set-rpath "${libPath}" $out/bin/${name}
            '';
          });
        };
        packages.opencas = naersk-lib.buildPackage rec {
          name = "opencas";
          src = ./.;
          doCheck = true;
          cargoBuildOptions = x: x ++ [ "-p" name ];
          cargoTestOptions = x: x ++ [ "-p" name ];
          doDoc = true;
          copyBins = false;
          doDocFail = true;
          copyTarget = true;
        };

        # nix run
        apps.advisory_viewer = utils.lib.mkApp rec {
          name = "advisory_viewer";
          drv = packages.${name};
        };

        # nix develop
        devShells.default = pkgs.mkShell {
          inputsFrom = with self.packages.${system}; [
            advisory_viewer
            opencas
          ];
          nativeBuildInputs = with pkgs; [
            cargo-flamegraph # for performance stuff
            toolchain # our rust toolchain
            trunk # for web stuff

            # keep everything neat
            nixpkgs-fmt
            nodePackages.prettier
            treefmt

            # dev stuff
          ];
          buildInputs = packages.advisory_viewer.buildInputs;
          LD_LIBRARY_PATH = libPath;
          shellHook = ''
            export IS_NIX_BUILD=true
          '';
        };
        checks = {
          treefmt = ((treefmt-nix.lib.evalModule pkgs treefmt').config.build.check self).overrideAttrs (o: {
            buildInputs = devShells.default.nativeBuildInputs ++ [ pkgs.git ];
          });
        };
      });
}
