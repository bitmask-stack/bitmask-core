# Crane fmt / clippy / nextest for bitmask-core.
#
# `just check` runs cargo fmt and cargo clippy on this machine, then
# remote native debug nextest (`nextest-quality`) and remote wasm nextest
# (`nextest-wasm-quality` in flake.nix). wasm32 stays in rust-toolchain.toml.
#
# Native nextest is `cargo nextest run` (dev profile, `--features server`).
# Docker / RGB-proxy / live HTTP / Esplora tests are skipped with
# sandboxSkipFilter. Document that skip in workspace RESIDUAL.md. Do not
# skip wasm nextest.
{
  lib,
  pkgs,
  craneLib,
  src,
  commonArgs,
  remote ? false,
}:
let
  remoteSystemFeatures = [
    "big-parallel"
    "surmount-remote"
  ];

  remoteAttrs = lib.optionalAttrs remote {
    preferLocalBuild = false;
    requiredSystemFeatures = remoteSystemFeatures;
  };

  pnameSuffix = if remote then "-quality" else "";

  # zstd-sys uses pkg-config when ZSTD_SYS_USE_PKG_CONFIG is *present*
  # (even =0). Unset it so the bundled zstd compiles. Do not unset
  # PKG_CONFIG: bitmask-core still needs pkg-config for OpenSSL.
  cargoJobsFromCores = ''
    cargoJobs="''${NIX_BUILD_CORES:-32}"
    case "$cargoJobs" in
      "" | *[!0-9]*) cargoJobs=32 ;;
    esac
    if [ "$cargoJobs" -gt 32 ]; then
      cargoJobs=32
    fi
    if [ "$cargoJobs" -lt 2 ]; then
      cargoJobs=32
    fi
    export CARGO_BUILD_JOBS="$cargoJobs"
    unset MAKEFLAGS MFLAGS CARGO_MAKEFLAGS
    unset ZSTD_SYS_USE_PKG_CONFIG
    echo "bitmask-core cargo jobs=$CARGO_BUILD_JOBS NIX_BUILD_CORES=''${NIX_BUILD_CORES:-unset}"
  '';

  qualityArgs = commonArgs // {
    enableParallelBuilding = true;
    CARGO_BUILD_JOBS = "32";
    preConfigure = ''
      unset ZSTD_SYS_USE_PKG_CONFIG
    '';
  };

  applyRemote = drv: drv.overrideAttrs (_: remoteAttrs);

  # Exclude-only. Nix sandbox has no docker compose, RGB-proxy, Lightning
  # HTTP, or live Esplora. Do not use this as an include-list of hermetic
  # names. wasm browser tests live under tests/ and are not this native run.
  sandboxSkipFilter = "not (binary(lightning) or binary(payjoin) or binary(nostr) or binary(migration) or test(/^rgb::server::/) or test(/^rgb::web::/) or test(/^bp::/) or test(/^taproot$/) or test(/^create_wallet$/) or test(/^import_wallet$/) or test(/^get_wallet_balance$/) or test(/^wrong_network$/))";

  clippyArtifacts = craneLib.buildDepsOnly (
    qualityArgs
    // remoteAttrs
    // {
      pname = "bitmask-core-clippy${pnameSuffix}";
      cargoExtraArgs = "--locked --all-features --all-targets";
      doCheck = false;
    }
  );

  fmt = applyRemote (
    craneLib.cargoFmt {
      inherit src;
      pname = "bitmask-core-fmt${pnameSuffix}";
      cargoExtraArgs = "--all";
    }
  );

  clippy = applyRemote (
    (craneLib.cargoClippy (
      qualityArgs
      // {
        cargoArtifacts = clippyArtifacts;
        pname = "bitmask-core-clippy${pnameSuffix}";
        cargoExtraArgs = "--locked --all-features --all-targets";
        # rustc errors stay -D warnings. async_fn_in_trait is allowed on the
        # RGB CrossStore traits in src/wallet/data.rs, not on this command.
        # unused/deprecated/style/complexity/perf command-line allows are gone.
        # Do not re-add --allow clippy::all or the style/complexity/perf groups.
        cargoClippyExtraArgs = "-- --deny warnings";
        doInstallCargoArtifacts = false;
      }
    )).overrideAttrs
      (old: {
        preBuild = (old.preBuild or "") + cargoJobsFromCores;
      })
  );

  nextest = applyRemote (
    (craneLib.cargoNextest (
      qualityArgs
      // {
        cargoArtifacts = clippyArtifacts;
        pname = "bitmask-core-nextest${pnameSuffix}";
        cargoExtraArgs = "--locked --features server";
        cargoNextestExtraArgs = "-E '${sandboxSkipFilter}' --no-fail-fast";
        CARGO_PROFILE = "dev";
        partitions = 1;
        partitionType = "count";
        nativeBuildInputs = (qualityArgs.nativeBuildInputs or [ ]) ++ [ pkgs.cargo-nextest ];
      }
    )).overrideAttrs
      (old: {
        preBuild =
          (old.preBuild or "")
          + cargoJobsFromCores
          + ''
            export HOME="$TMPDIR/home"
            export CARBONADO_DIR="$TMPDIR/carbonado"
            mkdir -p "$HOME" "$CARBONADO_DIR"
          '';
      })
  );
in
{
  inherit
    fmt
    clippy
    nextest
    sandboxSkipFilter
    ;
}
