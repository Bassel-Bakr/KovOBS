#!/usr/bin/env bash
# Makes the Linux AppImage work on rolling distros such as Arch and CachyOS, and adds a build that needs no FUSE.
#
# Tauri's AppImage bundles the build machine's libwayland-client, -cursor, -egl and -server. Against a newer Mesa on
# the user's system, WebKit then cannot create its EGL display ("Could not create default EGL display:
# EGL_BAD_PARAMETER. Aborting...") and the window stays blank. It happens under X11 too, and WebKit's environment
# switches do not help. Every desktop that runs GTK has its own libwayland, so the bundled copies are removed and the
# AppImage repacked. Reproduced and checked on CachyOS: the window is blank as built and renders with them gone.
#
# An AppImage also needs FUSE (a fusermount helper) to start, which not every system has. The same fixed app folder is
# packed as a .tar.gz, which runs from its AppRun without FUSE.
#
# Usage: scripts/linux-portable.sh <app.AppImage> <appimagetool> <out-dir> <name>
# Out: <out-dir>/<name>.AppImage (replacing any file of that name) and <out-dir>/<name>.tar.gz, which unpacks to <name>/.
set -euo pipefail

appimage=$(realpath "$1")
appimagetool=$(realpath "$2")
out=$(realpath "$3")
name=$4

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cd "$work" || exit 1

# Extracting needs no FUSE.
"$appimage" --appimage-extract > /dev/null

bundled=(squashfs-root/usr/lib/libwayland-*)
if [ -e "${bundled[0]}" ]; then
  echo "removing the bundled ${bundled[*]#squashfs-root/usr/lib/}"
  rm -f "${bundled[@]}"
else
  echo "no bundled libwayland found; repacking as is"
fi

# appimagetool is an AppImage itself, and CI runners have no FUSE either.
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$appimagetool" --no-appstream squashfs-root "$out/$name.AppImage"

mv squashfs-root "$name"
tar -czf "$out/$name.tar.gz" "$name"

echo "wrote $out/$name.AppImage and $out/$name.tar.gz"
