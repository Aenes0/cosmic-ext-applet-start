#!/bin/sh
# Build and install for the current user (default) or under another prefix:
#   ./install.sh            -> ~/.local
#   sudo ./install.sh /usr  -> system wide
set -e
cd "$(dirname "$0")"
prefix="${1:-$HOME/.local}"
id=io.github.Aenes0.CosmicStart
bin=cosmic-ext-applet-start

cargo build --release
install -Dm0755 "target/release/$bin" "$prefix/bin/$bin"
mkdir -p "$prefix/share/applications"
sed "s|@bindir@|$prefix/bin|" "data/$id.desktop.in" > "$prefix/share/applications/$id.desktop"
echo "Installed. Add \"Start\" in Settings > Desktop > Panel > Applets, or run ./setup.sh."
