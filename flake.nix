{
  description = "Pural development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
        };
      in
      {
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            python311
            uv

            # Native dependencies for Python wheels
            gcc
            zlib
            portaudio

            # Audio backends
            alsa-lib
            pipewire
          ];

          shellHook = ''
            export LD_LIBRARY_PATH="${
              pkgs.lib.makeLibraryPath [
                pkgs.stdenv.cc.cc
                pkgs.zlib
                pkgs.portaudio
                pkgs.alsa-lib
                pkgs.pipewire
              ]
            }:$LD_LIBRARY_PATH"

            export PKG_CONFIG_PATH="${
              pkgs.lib.makeSearchPath "lib/pkgconfig" [
                pkgs.portaudio
                pkgs.alsa-lib
                pkgs.pipewire
              ]
            }:$PKG_CONFIG_PATH"
          '';
        };
      }
    );
}
