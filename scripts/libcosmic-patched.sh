#!/bin/bash
# Builds the patched libcosmic that the start menu is compiled against.
#
# Usage:
#   scripts/libcosmic-patched.sh [--force]
#
# The Cargo.toml of this repository points a [patch] at ../libcosmic-patched.
# That directory is generated here, from:
#   * a pristine libcosmic checkout at commit d4d71fd (the one Cargo.lock pins),
#     found in the cargo git checkouts (cloned from GitHub if cargo's garbage
#     collector removed it), and
#   * the patches in third_party/libcosmic-patches/*.patch (the source of truth).
#
# The patches are paths relative to the `iced` submodule (iced/ has its own git
# repository), so they are applied with `git -C <dest>/iced apply`.
#
# Environment:
#   LIBCOSMIC_PATCHED_DIR  destination (default: libcosmic-patched next to this
#                          repository, i.e. ~/Projetos/libcosmic-patched)
#   CARGO_HOME             where cargo keeps its checkouts (default: ~/.cargo)
#
# Behaviour (idempotent; the origin checkout is never modified):
#   * no destination        -> copy the origin, apply the patches;
#   * patches already there -> print a message and exit 0, changing nothing;
#   * patches missing       -> apply them;
#   * diverged destination  -> exit 1 with an explanation. Nothing is deleted
#                              unless --force, which regenerates the directory
#                              (only if it looks like a libcosmic checkout).
set -eu

LIBCOSMIC_URL="https://github.com/pop-os/libcosmic"
LIBCOSMIC_REV="d4d71fd53e5ed6bd3a430089114dffa2da3cd498"
ICED_REV="511180679b3792387f9b5eabd6d11f4c1bc4ade6"

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_dir="$(dirname "$script_dir")"
patches_dir="$repo_dir/third_party/libcosmic-patches"
dest="${LIBCOSMIC_PATCHED_DIR:-$(dirname "$repo_dir")/libcosmic-patched}"
cargo_home="${CARGO_HOME:-$HOME/.cargo}"
force=0
work=""

die() {
    echo "libcosmic-patched: $*" >&2
    exit 1
}

cleanup() {
    if [ -n "$work" ] && [ -e "$work" ]; then
        rm -rf "$work"
    fi
}
trap cleanup EXIT

case "${1:-}" in
    "") ;;
    --force) force=1 ;;
    *) die "unknown argument '$1' (usage: libcosmic-patched.sh [--force])" ;;
esac

# True when $1 is libcosmic at LIBCOSMIC_REV with iced at ICED_REV.
is_pristine_source() {
    [ "$(git -C "$1" rev-parse HEAD 2>/dev/null)" = "$LIBCOSMIC_REV" ] &&
        [ "$(git -C "$1/iced" rev-parse HEAD 2>/dev/null)" = "$ICED_REV" ]
}

find_cargo_checkout() {
    local candidate
    for candidate in "$cargo_home"/git/checkouts/libcosmic-*/d4d71fd*; do
        if [ -d "$candidate" ] && is_pristine_source "$candidate"; then
            echo "$candidate"
            return 0
        fi
    done
    return 1
}

# Clones libcosmic into $1 at the pinned commits (fallback, needs network).
clone_source() {
    echo "libcosmic-patched: no cargo checkout found, cloning $LIBCOSMIC_URL"
    git clone --recurse-submodules "$LIBCOSMIC_URL" "$1"
    git -C "$1" checkout --quiet "$LIBCOSMIC_REV"
    git -C "$1" submodule update --init --recursive
    is_pristine_source "$1" ||
        die "the clone is not at libcosmic $LIBCOSMIC_REV / iced $ICED_REV"
}

list_patches() {
    local patch
    for patch in "$patches_dir"/*.patch; do
        [ -f "$patch" ] && echo "$patch"
    done
    return 0
}

# Prints "applied", "pending" or "diverged" for patch $2 inside tree $1.
patch_state() {
    if git -C "$1/iced" apply --reverse --check "$2" 2>/dev/null; then
        echo applied
    elif git -C "$1/iced" apply --check "$2" 2>/dev/null; then
        echo pending
    else
        echo diverged
    fi
}

# Applies every pending patch to tree $1. Returns 1 if any patch is diverged
# (nothing is applied in that case: all patches are checked first).
apply_patches() {
    local tree="$1" patch state pending=""
    for patch in $(list_patches); do
        state="$(patch_state "$tree" "$patch")"
        case "$state" in
            applied) echo "  already applied: $(basename "$patch")" ;;
            pending) pending="$pending $patch" ;;
            *)
                echo "libcosmic-patched: $tree has diverged: $(basename "$patch")" \
                    "neither applies nor is already applied" >&2
                return 1
                ;;
        esac
    done
    for patch in $pending; do
        git -C "$tree/iced" apply "$patch"
        echo "  applied: $(basename "$patch")"
    done
    if [ -z "$pending" ]; then
        return 2
    fi
    return 0
}

looks_like_libcosmic() {
    [ -f "$1/Cargo.toml" ] && grep -q '^name = "libcosmic"' "$1/Cargo.toml"
}

safe_to_remove() {
    case "$dest" in
        "" | / | "$HOME" | "$repo_dir" | "$HOME/Projetos") return 1 ;;
    esac
    case "$repo_dir/" in
        "$dest"/*) return 1 ;;
    esac
    looks_like_libcosmic "$dest"
}

update_existing() {
    local status=0
    apply_patches "$dest" || status=$?
    case "$status" in
        0) echo "libcosmic-patched: patches applied to $dest" ;;
        2) echo "libcosmic-patched: $dest is already patched, nothing to do" ;;
        *)
            echo "libcosmic-patched: rerun with --force to regenerate $dest" \
                "(it is deleted and recreated; only done for a libcosmic tree)" >&2
            return 1
            ;;
    esac
}

create_new() {
    local source
    work="$dest.tmp.$$"
    rm -rf "$work"
    if source="$(find_cargo_checkout)"; then
        echo "libcosmic-patched: copying $source"
        mkdir -p "$work"
        cp -a "$source/." "$work/"
        is_pristine_source "$work" || die "the copy of $source is not pristine"
    else
        clone_source "$work"
    fi
    apply_patches "$work" || die "could not apply the patches to a pristine tree"
    mv "$work" "$dest"
    work=""
    echo "libcosmic-patched: created $dest"
}

[ -n "$(list_patches)" ] || die "no patches in $patches_dir"

if [ -e "$dest" ] && [ "$force" = 1 ]; then
    safe_to_remove || die "refusing to --force over $dest (not a libcosmic tree)"
    rm -rf "$dest"
fi

if [ -e "$dest" ]; then
    looks_like_libcosmic "$dest" ||
        die "$dest exists but is not a libcosmic checkout (use another" \
            "LIBCOSMIC_PATCHED_DIR)"
    update_existing
else
    mkdir -p "$(dirname "$dest")"
    create_new
fi
