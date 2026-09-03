# bitmask-core quality gate. `just check` is local cargo fmt, local cargo
# clippy, then remote debug nextest and remote wasm nextest on Host
# surmount-1 (max-jobs 0, --store ssh-ng, --eval-store auto, --cores 64,
# fallback false). GitHub Actions must not call `just check` or
# `just check-remote`; GHA uses rust.yaml cargo. RGB is 0.11.1-rc.11.
# Carbonado is crates.io 0.7.0. RGB is not 0.12.

set shell := ["bash", "-euo", "pipefail", "-c"]

# Host system for flake check attributes. Prefer CI_SYSTEM. Do not call nix
# at just parse time.
system := env_var_or_default("CI_SYSTEM", `case "$(uname -s)-$(uname -m)" in Linux-x86_64) echo x86_64-linux;; Linux-aarch64|Linux-arm64) echo aarch64-linux;; Darwin-x86_64) echo x86_64-darwin;; Darwin-arm64) echo aarch64-darwin;; *) echo "unsupported $(uname -s)-$(uname -m); set CI_SYSTEM=..." >&2; exit 1;; esac`)

# Lists recipes. Does not run Nix.
default:
    @just --list

# Runs local cargo fmt and local cargo clippy on this machine. Does not run remote nextest. GitHub Actions uses rust.yaml cargo, not this recipe.
check-local:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "==> just check-local: cargo fmt --all"
    cargo fmt --all
    echo "==> just check-local: cargo clippy --all-targets --all-features -- --deny warnings"
    vendor=$(nix build --impure --print-out-paths --no-link "path:.#cargo-vendor")
    cargo_home=$(mktemp -d)
    export CARGO_HOME="${cargo_home}"
    if [[ -d target ]]; then chmod -R u+w target; fi
    cargo clippy --config "${vendor}/config.toml" --all-targets --all-features -- --deny warnings

# Runs local cargo fmt, local cargo clippy, then remote debug nextest and wasm nextest. GitHub Actions must not call this recipe.
check:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "==> just check: cargo fmt --all"
    cargo fmt --all
    echo "==> just check: cargo clippy --all-targets --all-features -- --deny warnings"
    vendor=$(nix build --impure --print-out-paths --no-link "path:.#cargo-vendor")
    cargo_home=$(mktemp -d)
    export CARGO_HOME="${cargo_home}"
    if [[ -d target ]]; then chmod -R u+w target; fi
    cargo clippy --config "${vendor}/config.toml" --all-targets --all-features -- --deny warnings
    export GROK_NIX_FORCE_REMOTE=1
    export GROK_NIX_BUILDERS_FILE="${GROK_NIX_BUILDERS_FILE:-$HOME/.config/nix/machines}"
    builders_file="${GROK_NIX_BUILDERS_FILE}"
    known_hosts="${GROK_NIX_KNOWN_HOSTS:-$HOME/.ssh/known_hosts}"
    extra_ssh="-o UserKnownHostsFile=${known_hosts} -o StrictHostKeyChecking=yes"
    if [[ -n "${NIX_SSHOPTS:-}" ]]; then
      export NIX_SSHOPTS="${NIX_SSHOPTS} ${extra_ssh}"
    else
      export NIX_SSHOPTS="${extra_ssh}"
    fi
    if [[ ! -s "${builders_file}" ]]; then
      echo "The Nix builders file is missing or empty: ${builders_file}." >&2
      echo "just check reuses the trusted-user machines file (override with GROK_NIX_BUILDERS_FILE)." >&2
      echo "just check-local does not need this file." >&2
      exit 2
    fi
    store_uri=""
    while IFS= read -r line || [[ -n "${line}" ]]; do
      if [[ "${line}" == ssh-ng://* ]]; then
        read -r store_uri _ <<<"${line}" || true
        break
      fi
    done < "${builders_file}"
    if [[ -z "${store_uri}" || "${store_uri}" != ssh-ng://* ]]; then
      echo "The Nix builders file ${builders_file} has no ssh-ng:// builder line." >&2
      echo "just check will not fall back to local Nix store builds." >&2
      exit 2
    fi
    if [[ "${store_uri}" != *"max-connections="* ]]; then
      if [[ "${store_uri}" == *\?* ]]; then
        store_uri="${store_uri}&max-connections=8"
      else
        store_uri="${store_uri}?max-connections=8"
      fi
    fi
    nix_quality() {
      local attr="$1"
      echo "==> just check: ${attr}"
      extra=()
      if [[ "${GROK_NIX_REBUILD:-}" == 1 ]]; then
        extra+=(--rebuild)
        echo "==> just check: --rebuild (do not reuse an existing nextest store path)"
      fi
      nix build --impure -L --print-out-paths --no-link \
        --option builders "@${builders_file}" \
        --option builders-use-substitutes true \
        --option fallback false \
        --option system-features "kvm nixos-test uid-range" \
        --option max-jobs 0 \
        --cores 64 \
        --store "${store_uri}" \
        --eval-store auto \
        "${extra[@]}" \
        "path:.#${attr}"
    }
    nix_quality nextest-quality
    nix_quality nextest-wasm-quality

# Runs the same local fmt, local clippy, and remote nextest gate as check. GitHub Actions must not call this recipe.
check-remote: check

# Rebuilds remote debug nextest and wasm nextest even if a matching store path already exists, after local fmt and local clippy.
check-rebuild:
    GROK_NIX_REBUILD=1 just check
