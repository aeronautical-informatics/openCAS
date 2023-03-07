{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    utils.url = "github:numtide/flake-utils";
    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";
    naersk.url = "github:nix-community/naersk";
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
        requiredLibs = with pkgs;
          [
            libGL
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
            binaryen
            httplz
            binaryen
            toolchain
            cargo-flamegraph
            (wasm-bindgen-cli.overrideAttrs (prev: rec {
              version = "0.2.82";
              src = pkgs.fetchCrate {
                inherit version;
                inherit (prev) pname;
                sha256 = "sha256-BQ8v3rCLUvyCCdxo5U+NHh30l9Jwvk9Sz8YQv6fa0SU=";
              };
              cargoDeps = prev.cargoDeps.overrideAttrs (lib.const {
                name = "${prev.pname}-vendor.tar.gz";
                inherit src;
                outputHash =
                  "sha256-ACo4zG+JK/fW6f9jpfTLPDUYqPqrt9Q2XgCF26jBXkg=";
              });
            }))
          ];
          buildInputs = packages.advisory_viewer.buildInputs;
          LD_LIBRARY_PATH = libPath;
          shellHook = ''
            export IS_NIX_BUILD=true
          '';
        };
        checks = {
          format = pkgs.runCommand "check-format"
            {
              nativeBuildInputs = [ pkgs.nixpkgs-fmt toolchain ];
            } ''
            nixpkgs-fmt --check ${./.}
            ( cd ${./.} && cargo fmt --check )
            touch $out
          '';
          # clippy = pkgs.runCommand "clippy"
          #   {
          #     nativeBuildInputs = [ toolchain ];
          #   } ''
          #   ( cd ${./.} && cargo clippy )
          #   touch $out
          # '';

        };
      });
}
