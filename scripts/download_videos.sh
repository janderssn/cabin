#!/usr/bin/env bash
set -euo pipefail
# Usage: download_videos.sh /absolute/path/to/sources.txt
sources=$(realpath "$1")
mkdir -p "$HOME/videos/flight/documentaries"
cd "$HOME/videos/flight/documentaries"
yt-dlp --no-playlist --continue --write-info-json --write-description --write-thumbnail \
  --write-subs --write-auto-subs --sub-langs 'en,en-orig' --sub-format 'vtt/srt/best' \
  -f 'bv*[height<=720]+ba/b[height<=720]' --merge-output-format mkv \
  -o '%(title)s [%(id)s].%(ext)s' -a "$sources"
