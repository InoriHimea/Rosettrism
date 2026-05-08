#!/usr/bin/env sh
set -eu

alias_mode="symlink"

usage() {
    cat <<'EOF'
Usage: scripts/build-release.sh [--alias-mode symlink|hardlink]

Builds the rosettrism release binary and creates lightweight command aliases:
  symlink  -> target/release/rstm and target/release/rosm point to rosettrism
  hardlink -> target/release/rstm and target/release/rosm share rosettrism's data
EOF
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --alias-mode)
            if [ "$#" -lt 2 ]; then
                echo "missing value for --alias-mode" >&2
                exit 2
            fi
            alias_mode="$2"
            shift 2
            ;;
        --alias-mode=*)
            alias_mode="${1#*=}"
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "unknown option: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

case "$alias_mode" in
    symlink|hardlink) ;;
    *)
        echo "unsupported alias mode: $alias_mode" >&2
        usage >&2
        exit 2
        ;;
esac

script_dir=$(CDPATH= cd "$(dirname "$0")" && pwd)
root=$(CDPATH= cd "$script_dir/.." && pwd)
release_dir="$root/target/release"
binary_path="$release_dir/rosettrism"

cd "$root"
cargo build --release --bin rosettrism

if [ ! -f "$binary_path" ] && [ -f "$binary_path.exe" ]; then
    binary_path="$binary_path.exe"
fi

if [ ! -f "$binary_path" ]; then
    echo "expected release binary was not found: $binary_path" >&2
    exit 1
fi

binary_name=$(basename "$binary_path")

for alias_name in rstm rosm; do
    alias_path="$release_dir/$alias_name"

    rm -f "$alias_path" "$alias_path.exe" "$alias_path.cmd"

    if [ "$alias_mode" = "hardlink" ]; then
        ln "$binary_path" "$alias_path"
    else
        ln -s "$binary_name" "$alias_path"
    fi
done

echo "Built $binary_path"
if [ "$alias_mode" = "hardlink" ]; then
    echo "Created hard-link aliases: rstm, rosm"
else
    echo "Created symlink aliases: rstm, rosm"
fi
