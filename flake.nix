{
  inputs = {
    utils.url = "github:numtide/flake-utils";
    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";
    naersk.url = "github:nmattia/naersk";
    naersk.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, utils, fenix, naersk }:
    utils.lib.eachDefaultSystem (system:
      let
        lib = nixpkgs.lib;
        pkgs = nixpkgs.legacyPackages."${system}";
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
      in
      rec {
        # nix build
        packages.advisory_viewer =
          let
            rpathLibs = with pkgs; [
              libGL
              xorg.libX11
              xorg.libXcursor
              xorg.libXi
              xorg.libXrandr
              xorg.libXxf86vm
              xorg.libxcb
            ] ++ lib.optionals stdenv.isLinux [
              libxkbcommon
              wayland
            ];
          in
          naersk-lib.buildPackage rec {
            name = "advisory_viewer";
            src = ./.;
            doCheck = true;
            cargoBuildOptions = x: x ++ [ "-p" name ];
            cargoTestOptions = x: x ++ [ "-p" name ];
            #nativeBuildInputs = with pkgs; [ pkg-config ];
            overrideMain = (_: {
              postFixup = ''
                patchelf --set-rpath "${lib.makeLibraryPath rpathLibs}" $out/bin/${name}
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
        apps.advisory_viewer = utils.lib.mkApp rec { name = "advisory_viewer"; drv = packages.${name}; };

        # nix develop
        devShells.default = pkgs.mkShell {
          inputsFrom = with self.packages.${system}; [ advisory_viewer opencas ];
          nativeBuildInputs = with pkgs; [
            binaryen
            httplz
            binaryen
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
        checks = {
          format = pkgs.runCommand "check-format"
            {
              nativeBuildInputs = [ pkgs.nixpkgs-fmt toolchain ];
            } ''
            nixpkgs-fmt --check ${./.}
            cargo fmt --check
            touch $out
          '';
          clippy = pkgs.runCommand "clippy"
            {
              nativeBuildInputs = [ toolchain ];
            } ''
            cargo clippy
            touch $out
          '';

        };

      });
}
