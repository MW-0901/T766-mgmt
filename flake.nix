{
  description = "A Nix-flake-based Rust development environment";

  inputs = {
    nixpkgs.url = "https://flakehub.com/f/NixOS/nixpkgs/0.1"; # unstable Nixpkgs
    fenix = {
      url = "https://flakehub.com/f/nix-community/fenix/0.1";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { self, ... }@inputs:

    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forEachSupportedSystem =
        f:
        inputs.nixpkgs.lib.genAttrs supportedSystems (
          system:
          f {
            pkgs = import inputs.nixpkgs {
              inherit system;
              overlays = [
                inputs.self.overlays.default
              ];
            };
          }
        );
    in
    {
      overlays.default = final: prev: {
        rustToolchain =
          with inputs.fenix.packages.${prev.stdenv.hostPlatform.system};
          combine (
            with stable;
            [
              clippy
              rustc
              cargo
              rustfmt
              rust-src
              targets.wasm32-unknown-unknown.stable.rust-std
            ]
          );
      };

      devShells = forEachSupportedSystem (
        { pkgs }:
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              rustToolchain
              openssl
              pkg-config
              cargo-deny
              cargo-edit
              cargo-watch
              rust-analyzer
              gnumake
              dioxus-cli
            ];

            env = {
              # Required by rust-analyzer
              RUST_SRC_PATH = "${pkgs.rustToolchain}/lib/rustlib/src/rust/library";
            };
          };
        }
      );
      apps = forEachSupportedSystem (
        { pkgs }:
        {
          default = {
            type = "app";
            program = "${pkgs.writeShellApplication {
              name = "run-control-server";
              runtimeInputs = [ pkgs.dioxus-cli ];
              text = ''
                if [ ! -d /puppet ]; then
                  echo "Please create /puppet directory!"
                  exit 1
                fi

                if [ ! -w /puppet ]; then
                  echo "The /puppet directory must be writable!"
                  exit 1
                fi

                cd T766-ControlServer
                dx serve --release --port 8000
              '';
            }}/bin/run-control-server";
          };
          serve = {
            type = "app";
            program = "${pkgs.writeShellApplication {
              name = "serve-prod";
              runtimeInputs = with pkgs; [ rustToolchain dioxus-cli ];
              text = ''
                if [ ! -d /puppet ]; then
                  echo "Please create /puppet directory!"
                  exit 1
                fi

                if [ ! -w /puppet ]; then
                  echo "The /puppet directory must be writable!"
                  exit 1
                fi

                cd T766-ControlServer
                
                if [ ! -f target/release/T766-ControlServer ] || [ src -nt target/release/T766-ControlServer ]; then
                  dx bundle --release
                fi

                PORT=8000 ./target/release/T766-ControlServer
              '';
            }}/bin/serve-prod";
          };
        }
      );
    };
}
