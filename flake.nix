{
  description = "T766 Laptop Management System";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
  };

  outputs = { self, nixpkgs, fenix, crane }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
      fenixPkgs = fenix.packages.${system};

      # Single Rust toolchain with all needed targets
      rustToolchain = fenixPkgs.combine [
        fenixPkgs.stable.rustc
        fenixPkgs.stable.cargo
        fenixPkgs.stable.clippy
        fenixPkgs.stable.rust-src
        fenixPkgs.targets.wasm32-unknown-unknown.stable.rust-std
        fenixPkgs.targets.aarch64-unknown-linux-gnu.stable.rust-std
        fenixPkgs.targets.x86_64-pc-windows-gnu.stable.rust-std
      ];

      craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

      # wasm-bindgen-cli pinned to match Cargo.lock (0.2.118)
      wasm-bindgen-cli = pkgs.buildWasmBindgenCli rec {
        src = pkgs.fetchCrate {
          pname = "wasm-bindgen-cli";
          version = "0.2.118";
          hash = "sha256-ve783oYH0TGv8Z8lIPdGjItzeLDQLOT5uv/jbFOlZpI=";
        };
        cargoDeps = pkgs.rustPlatform.fetchCargoVendor {
          inherit src;
          inherit (src) pname version;
          hash = "sha256-EYDfuBlH3zmTxACBL+sjicRna84CvoesKSQVcYiG9P0=";
        };
      };

      # Source filtering - include Rust, config, and asset files
      src = pkgs.lib.cleanSourceWith {
        src = ./.;
        filter = path: type:
          (craneLib.filterCargoSources path type)
          || (builtins.match ".*\\.(css|html|js|svg|ico|slint|nsi)$" path != null)
          || (builtins.match ".*/assets/.*" path != null)
          || (builtins.match ".*/ui/.*" path != null)
          || (builtins.match ".*/Dioxus\\.toml$" path != null)
          || (builtins.match ".*/Packager\\.toml$" path != null)
          || (builtins.baseNameOf path == "settings.toml");
      };

      # Vendor all cargo dependencies for offline/sandboxed builds
      cargoVendorDir = craneLib.vendorCargoDeps { inherit src; };

      # Helper: generate .cargo/config.toml with vendored deps + optional extras
      mkCargoConfig = extraToml: ''
        mkdir -p .cargo
        cp ${cargoVendorDir}/config.toml .cargo/config.toml
        chmod +w .cargo/config.toml
        cat >> .cargo/config.toml <<'EXTRAEOF'
        ${extraToml}
        EXTRAEOF

        export HOME=$(mktemp -d)
        export CARGO_HOME="$HOME/.cargo"
      '';

      # Cross-compilation package sets
      pkgsAarch64 = pkgs.pkgsCross.aarch64-multiplatform;

      # Common build inputs for ControlServer
      commonNativeBuildInputs = [
        rustToolchain
        pkgs.dioxus-cli
        pkgs.binaryen
        wasm-bindgen-cli
        pkgs.pkg-config
      ];

      # ================================================================
      # Control Server - x86_64-linux
      # ================================================================
      controlServer-x86_64 = pkgs.stdenv.mkDerivation {
        pname = "T766-ControlServer-x86_64";
        version = "0.1.0";
        inherit src;

        nativeBuildInputs = commonNativeBuildInputs;
        buildInputs = [ pkgs.openssl ];

        configurePhase = mkCargoConfig "";

        buildPhase = ''
          dx bundle --release -p T766-ControlServer
        '';

        installPhase = ''
          mkdir -p $out/bin
          cp -r target/dx/T766-ControlServer/release/web $out/web
          cp target/dx/T766-ControlServer/release/web/server $out/bin/T766-ControlServer
        '';
      };

      # ================================================================
      # Control Server - aarch64-linux (Raspberry Pi)
      # The WASM frontend is architecture-independent, so we reuse it
      # from the x86_64 build and only cross-compile the server binary.
      # ================================================================
      controlServer-aarch64 = pkgs.stdenv.mkDerivation {
        pname = "T766-ControlServer-rpi";
        version = "0.1.0";
        inherit src;

        nativeBuildInputs = [
          rustToolchain
          pkgs.pkg-config
          pkgsAarch64.stdenv.cc
        ];

        buildInputs = [
          pkgsAarch64.openssl
        ];

        configurePhase = mkCargoConfig ''
          [target.aarch64-unknown-linux-gnu]
          linker = "${pkgsAarch64.stdenv.cc}/bin/${pkgsAarch64.stdenv.cc.targetPrefix}cc"
        '';

        CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER =
          "${pkgsAarch64.stdenv.cc}/bin/${pkgsAarch64.stdenv.cc.targetPrefix}cc";
        # Tell the cc crate to use the cross-compiler for C/asm (needed by ring)
        CC_aarch64_unknown_linux_gnu =
          "${pkgsAarch64.stdenv.cc}/bin/${pkgsAarch64.stdenv.cc.targetPrefix}cc";
        AR_aarch64_unknown_linux_gnu =
          "${pkgsAarch64.stdenv.cc}/bin/${pkgsAarch64.stdenv.cc.targetPrefix}ar";
        PKG_CONFIG_ALLOW_CROSS = "1";
        PKG_CONFIG_PATH = "${pkgsAarch64.openssl.dev}/lib/pkgconfig";

        buildPhase = ''
          cargo build --release --target aarch64-unknown-linux-gnu \
            -p T766-ControlServer --features server --no-default-features
        '';

        installPhase = ''
          mkdir -p $out/bin $out/web/public
          cp target/aarch64-unknown-linux-gnu/release/T766-ControlServer $out/bin/
          # Reuse WASM frontend assets from the x86_64 build
          cp -r ${controlServer-x86_64}/web/public/* $out/web/public/
          cp $out/bin/T766-ControlServer $out/web/server
        '';
      };

      # ================================================================
      # Windows Client - cross-compiled .exe files
      # ================================================================
      pkgsWindows = pkgs.pkgsCross.mingwW64;

      windowsClient = pkgs.stdenv.mkDerivation {
        pname = "T766-ControlClient-windows";
        version = "0.1.0";
        inherit src;

        nativeBuildInputs = [
          rustToolchain
          pkgsWindows.stdenv.cc
          pkgs.nsis
        ];

        buildInputs = [
          pkgsWindows.windows.pthreads
        ];

        configurePhase = mkCargoConfig ''
          [target.x86_64-pc-windows-gnu]
          linker = "${pkgsWindows.stdenv.cc}/bin/${pkgsWindows.stdenv.cc.targetPrefix}cc"
          rustflags = ["-L", "native=${pkgsWindows.windows.pthreads}/lib"]
        '';

        CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER =
          "${pkgsWindows.stdenv.cc}/bin/${pkgsWindows.stdenv.cc.targetPrefix}cc";
        CC_x86_64_pc_windows_gnu =
          "${pkgsWindows.stdenv.cc}/bin/${pkgsWindows.stdenv.cc.targetPrefix}cc";
        AR_x86_64_pc_windows_gnu =
          "${pkgsWindows.stdenv.cc}/bin/${pkgsWindows.stdenv.cc.targetPrefix}ar";

        buildPhase = ''
          cargo build --release --target x86_64-pc-windows-gnu \
            -p T766-ControlClient \
            -p T766-CheckinApp
        '';

        installPhase = ''
          mkdir -p $out staging
          cp target/x86_64-pc-windows-gnu/release/T766-ControlClient.exe staging/
          cp target/x86_64-pc-windows-gnu/release/T766-CheckinApp.exe staging/
          cp settings.toml staging/ 2>/dev/null || true

          # Build NSIS installer (copy .nsi into staging so File directives resolve)
          cp installer.nsi staging/
          pushd staging
          makensis installer.nsi
          popd
          cp staging/T766-ControlClient-Installer.exe $out/

          # Also keep the raw exes available
          mkdir -p $out/bin
          cp staging/T766-ControlClient.exe staging/T766-CheckinApp.exe $out/bin/
          cp staging/settings.toml $out/bin/ 2>/dev/null || true
        '';
      };

    in {
      packages.x86_64-linux = {
        controlServer = controlServer-x86_64;
        controlServer-rpi = controlServer-aarch64;
        windowsClient = windowsClient;
        default = controlServer-x86_64;
      };

      devShells.x86_64-linux.default = pkgs.mkShell {
        buildInputs = [
          rustToolchain
          pkgs.dioxus-cli
          pkgs.binaryen
          wasm-bindgen-cli
          pkgs.pkg-config
          pkgs.openssl
          pkgs.nsis
        ];
      };
    };
}
