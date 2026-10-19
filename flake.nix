{
  description = "BitMask Core: Bitcoin, RGB, and wasm32 wallet library";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    # Binary Rust 1.99.0 via rust-overlay (fromRustupToolchainFile).
    # Manifest: https://github.com/oxalica/rust-overlay/blob/master/manifests/stable/1.99.0.nix
    # (dated 2026-10-01; accessed: 2026-10-09).
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      crane,
      advisory-db,
    }:
    let
      inherit (nixpkgs) lib;
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];
      forAllSystems = lib.genAttrs systems;

      # Crane keys these by Cargo.lock `source =` strings, not crate names.
      # Prefetched 2026-08-24 with `nix flake prefetch git+https://...?rev=`.
      # Carbonado 0.7.0 and bao-tree 0.16.1 are crates.io (no git FOD).
      gitOutputHashes = {
        "git+https://github.com/BP-WG/bp-core?rev=65394dd178c121eabb6a98b2e0c907d5b558b1c8#65394dd178c121eabb6a98b2e0c907d5b558b1c8" =
          "sha256-ZdpWNYf+E8E0RNbaN4Wj7/SrasIW1TEUt5aC119XdnQ=";
        "git+https://github.com/zoedberg/bp-wallet?rev=e472fe049791b2db6178a8ca592114e127a5dd12#e472fe049791b2db6178a8ca592114e127a5dd12" =
          "sha256-Mz3PY25NXBSQ8JfYvnzbmUX/z+I+F2DzVM2Glf1GhqU=";
        "git+https://github.com/LNP-BP/client_side_validation?rev=f896834228cc6dddba0b9ca0337553c41c205cf5#f896834228cc6dddba0b9ca0337553c41c205cf5" =
          "sha256-xBoBFd/pKJB2/oYaucRK0reTdF62eLI2UkLLUqOwX84=";
      };

      perSystem = forAllSystems (
        system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };

          # rust-toolchain.toml is the pin (channel 1.99.0 + clippy/rustfmt + wasm32).
          rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

          src = lib.fileset.toSource {
            root = ./.;
            fileset = lib.fileset.unions [
              ./Cargo.toml
              ./Cargo.lock
              ./rust-toolchain.toml
              ./build.rs
              ./RGB_LIB_IDs.toml
              ./file_hashes.toml
              ./.env
              ./src
              ./tests
              # wasm Network helper text only. Not a wasm compile.
              ./lib/web/constants.ts
            ];
          };

          cargoVendorDir = craneLib.vendorCargoDeps {
            inherit src;
            outputHashes = gitOutputHashes;
            # Crane's git vendor runs `cargo package -l` on every Cargo.toml
            # in remaining RGB git checkouts. cargo 1.99 treats Cargo.toml.orig
            # as reserved. Keep this off commonArgs: mkCargoDerivation would
            # coerce it into a derivation env var.
            overrideVendorGitCheckout =
              _ps: drv:
              drv.overrideAttrs (old: {
                postPatch = (old.postPatch or "") + ''
                  find . -name Cargo.toml.orig -delete
                '';
              });
          };

          commonArgs = {
            inherit src cargoVendorDir;
            pname = "bitmask-core";
            version = "0.12.0";
            strictDeps = true;
            outputHashes = gitOutputHashes;
            nativeBuildInputs = [
              pkgs.pkg-config
            ];
            buildInputs = [
              pkgs.openssl
            ]
            ++ lib.optionals pkgs.stdenv.hostPlatform.isDarwin [
              pkgs.libiconv
            ];
            # Harmless leftover env. nostr 0.44.8 has no GIT_HASH build.rs.
            GIT_HASH = "vendored";
          };

          cargoArtifacts = craneLib.buildDepsOnly (
            commonArgs
            // {
              cargoExtraArgs = "--locked --features server";
            }
          );

          bitmaskd = craneLib.buildPackage (
            commonArgs
            // {
              inherit cargoArtifacts;
              pname = "bitmaskd";
              cargoExtraArgs = "--locked --features server";
              doCheck = false;
              meta = {
                description = "BitMask Core REST node (bitmaskd)";
                homepage = "https://bitmask.app";
                license = lib.licenses.mit;
                mainProgram = "bitmaskd";
              };
            }
          );

          # Lock wasm-bindgen is 0.2.129. nixpkgs wasm-bindgen-cli is older.
          wasmBindgenCli = pkgs.buildWasmBindgenCli rec {
            src = pkgs.fetchCrate {
              pname = "wasm-bindgen-cli";
              version = "0.2.129";
              hash = "sha256-pcecKQd7E8Opw6bkFoE569epUi7gh5qpQF1e5PJY6V8=";
            };
            cargoDeps = pkgs.rustPlatform.fetchCargoVendor {
              inherit src;
              inherit (src) pname version;
              hash = "sha256-vmUrWVU7kPJJxO5qIVeAkwQyWDELO1Z4Z5gitz2kco8=";
            };
          };

          # secp256k1-sys / secp256k1-zkp compile C for wasm32 via `cc`.
          # llvm-binutils-wrapper has `ar`, not `llvm-ar`. cc-rs looks for
          # `$AR_wasm32_unknown_unknown` as a real path, so point at unwrapped
          # llvm-ar. Wrapper path that failed: .../llvm-binutils-wrapper-21.1.8/bin/llvm-ar.
          wasmCc = {
            nativeBuildInputs = [
              pkgs.pkg-config
              pkgs.llvmPackages.clang
              pkgs.llvmPackages.clang-unwrapped
              pkgs.llvmPackages.bintools
              pkgs.llvmPackages.bintools-unwrapped
              pkgs.lld
            ];
            # Unwrapped clang: nix cc-wrapper injects
            # -fzero-call-used-regs=used-gpr, which wasm32 rejects.
            CC_wasm32_unknown_unknown = "${pkgs.llvmPackages.clang-unwrapped}/bin/clang";
            CFLAGS_wasm32_unknown_unknown = "--target=wasm32-unknown-unknown";
            AR_wasm32_unknown_unknown = "${pkgs.llvmPackages.bintools-unwrapped}/bin/llvm-ar";
            RANLIB_wasm32_unknown_unknown = "${pkgs.llvmPackages.bintools-unwrapped}/bin/llvm-ranlib";
            CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER = "${pkgs.lld}/bin/wasm-ld";
            # getrandom 0.3 wasm_js backend. See bitmask-next build.sh
            # (accessed: 2026-08-25).
            CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS = "--cfg getrandom_backend=\"wasm_js\"";
          };

          wasmArtifacts = craneLib.buildDepsOnly (
            commonArgs
            // wasmCc
            // {
              pname = "bitmask-core-wasm";
              cargoExtraArgs = "--locked --lib --target wasm32-unknown-unknown";
              CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
              doCheck = false;
            }
          );

          bitmask-core-wasm = craneLib.buildPackage (
            commonArgs
            // wasmCc
            // {
              cargoArtifacts = wasmArtifacts;
              pname = "bitmask-core-wasm";
              cargoExtraArgs = "--locked --lib --target wasm32-unknown-unknown";
              CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
              doCheck = false;
              meta = {
                description = "BitMask Core wasm32-unknown-unknown library";
                homepage = "https://bitmask.app";
                license = lib.licenses.mit;
              };
            }
          );

          mkCargoQuality =
            remote:
            import ./nix/cargo-quality.nix {
              inherit
                lib
                pkgs
                craneLib
                src
                commonArgs
                remote
                ;
            };
          cargoChecks = mkCargoQuality false;
          cargoQuality = mkCargoQuality true;

          clippy-wasm = craneLib.cargoClippy (
            commonArgs
            // wasmCc
            // {
              cargoArtifacts = wasmArtifacts;
              cargoExtraArgs = "--locked --lib --tests --target wasm32-unknown-unknown";
              cargoClippyExtraArgs = "-- --deny warnings";
              CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
            }
          );

          # Optional remote wasm clippy. If secp256k1-sys still fails, stop
          # and quote the compiler line; keep wasm32 in rust-toolchain.toml.
          stripZeroCallUsedRegs = cflags: lib.concatStringsSep " " (
            lib.filter (f: f != "" && !(lib.hasInfix "zero-call-used-regs" f)) (
              lib.splitString " " cflags
            )
          );
          wasmQualityRemote = {
            preferLocalBuild = false;
            requiredSystemFeatures = [
              "big-parallel"
              "surmount-remote"
            ];
          };
          wasmQualityArtifacts = (craneLib.buildDepsOnly (
            commonArgs
            // wasmCc
            // wasmQualityRemote
            // {
              pname = "bitmask-core-wasm-quality";
              cargoExtraArgs = "--locked --lib --target wasm32-unknown-unknown";
              CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
              doCheck = false;
            }
          )).overrideAttrs (old: {
            hardeningDisable = [ "all" ];
            NIX_CFLAGS_COMPILE = stripZeroCallUsedRegs (old.NIX_CFLAGS_COMPILE or "");
          });
          clippy-wasm-quality = (craneLib.cargoClippy (
            commonArgs
            // wasmCc
            // wasmQualityRemote
            // {
              cargoArtifacts = wasmQualityArtifacts;
              pname = "bitmask-core-clippy-wasm-quality";
              cargoExtraArgs = "--locked --lib --target wasm32-unknown-unknown";
              cargoClippyExtraArgs = "-- --deny warnings";
              CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
            }
          )).overrideAttrs (old: {
            hardeningDisable = [ "all" ];
            NIX_CFLAGS_COMPILE = stripZeroCallUsedRegs (old.NIX_CFLAGS_COMPILE or "");
          });

          # Remote wasm32-unknown-unknown nextest (lib tests, debug profile).
          # Not `server` / tokio full. Browser RGB/web integration tests stay
          # GHA wasm-pack. Runner is wasm-bindgen-test-runner (llvm-ar from wasmCc).
          nextest-wasm-quality = (craneLib.cargoNextest (
            commonArgs
            // wasmCc
            // wasmQualityRemote
            // {
              cargoArtifacts = wasmQualityArtifacts;
              pname = "bitmask-core-nextest-wasm-quality";
              cargoExtraArgs = "--locked --lib --target wasm32-unknown-unknown";
              # wasm-bindgen-test-runner lists #[wasm_bindgen_test] only.
              # --no-tests=pass: empty list is still a compiled lib-test binary.
              cargoNextestExtraArgs = "--no-fail-fast --no-tests=pass";
              CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
              CARGO_PROFILE = "dev";
              partitions = 1;
              partitionType = "count";
              nativeBuildInputs = (commonArgs.nativeBuildInputs or [ ]) ++ [
                pkgs.cargo-nextest
                wasmBindgenCli
                pkgs.nodejs
              ];
              CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER = "${wasmBindgenCli}/bin/wasm-bindgen-test-runner";
            }
          )).overrideAttrs (old: {
            hardeningDisable = [ "all" ];
            NIX_CFLAGS_COMPILE = stripZeroCallUsedRegs (old.NIX_CFLAGS_COMPILE or "");
            preBuild =
              (old.preBuild or "")
              + ''
                unset ZSTD_SYS_USE_PKG_CONFIG
                export HOME="$TMPDIR/home"
                mkdir -p "$HOME"
              '';
          });

          audit = craneLib.cargoAudit {
            inherit src advisory-db;
          };

          devShell = pkgs.mkShell {
            packages = [
              rustToolchain
              pkgs.pkg-config
              pkgs.openssl
              pkgs.cargo-nextest
              pkgs.cargo-audit
              pkgs.wasm-pack
              pkgs.just
            ];
            PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
          };
        in
        {
          inherit
            bitmaskd
            bitmask-core-wasm
            cargoVendorDir
            clippy-wasm
            clippy-wasm-quality
            nextest-wasm-quality
            audit
            devShell
            rustToolchain
            ;
          fmt = cargoChecks.fmt;
          clippy = cargoChecks.clippy;
          nextest = cargoChecks.nextest;
          fmt-quality = cargoQuality.fmt;
          clippy-quality = cargoQuality.clippy;
          nextest-quality = cargoQuality.nextest;
        }
      );
    in
    {
      packages = forAllSystems (system: {
        default = perSystem.${system}.bitmaskd;
        bitmaskd = perSystem.${system}.bitmaskd;
        bitmask-core-wasm = perSystem.${system}.bitmask-core-wasm;
        # Lockfile vendor for local cargo clippy. Not a crates.io skip of menhera-cooldown.
        cargo-vendor = perSystem.${system}.cargoVendorDir;
        # cargo-audit of the RGB 0.11 stack. Not a flake check: remaining
        # advisories live in SECURITY.md, not an allow-lint.
        audit = perSystem.${system}.audit;
        # Named Crane packages. `just check` runs local cargo fmt/clippy,
        # then remote nextest-quality and nextest-wasm-quality. Not used by GHA.
        fmt-quality = perSystem.${system}.fmt-quality;
        clippy-quality = perSystem.${system}.clippy-quality;
        nextest-quality = perSystem.${system}.nextest-quality;
        nextest-wasm-quality = perSystem.${system}.nextest-wasm-quality;
        clippy-wasm-quality = perSystem.${system}.clippy-wasm-quality;
      });

      checks = forAllSystems (system: {
        bitmaskd = perSystem.${system}.bitmaskd;
        fmt = perSystem.${system}.fmt;
        clippy = perSystem.${system}.clippy;
        nextest = perSystem.${system}.nextest;
      });

      devShells = forAllSystems (system: {
        default = perSystem.${system}.devShell;
      });

      formatter = forAllSystems (system: nixpkgs.legacyPackages.${system}.nixfmt-rfc-style);
    };
}
