#!/usr/bin/env bash
# Turn a timed frame capture into an MP4 and a looping GIF.
set -euo pipefail
CAP="${1:-/tmp/demo/cap}"
OUT="${2:-/tmp/demo/out}"
LIST="$(mktemp -d)/frames.txt"
FPS="${FPS:-20}"
GIF_FPS="${GIF_FPS:-14}"
GIF_W="${GIF_W:-920}"

mkdir -p "$OUT"
# Real capture timing -> a constant-rate timeline ffmpeg can read.
python3 - "$CAP" "$LIST" <<'PY'
import json, sys, os
cap, list_path = sys.argv[1], sys.argv[2]
man = json.load(open(os.path.join(cap, "manifest.json")))
lines = []
for f in man:
    lines.append(f"file '{f['file']}'")
    lines.append(f"duration {max(f['dur'],20)/1000:.4f}")
lines.append(f"file '{man[-1]['file']}'")   # concat needs the last frame twice
open(list_path, "w").write("\n".join(lines) + "\n")
print(f"{len(man)} frames, {sum(f['dur'] for f in man)/1000:.1f}s")
PY

ffmpeg -y -loglevel error -f concat -safe 0 -i "$LIST" \
  -vf "fps=$FPS,scale=trunc(iw/2)*2:trunc(ih/2)*2:flags=lanczos" \
  -c:v libx264 -preset slow -crf 20 -pix_fmt yuv420p -movflags +faststart \
  "$OUT/demo.mp4"

ffmpeg -y -loglevel error -f concat -safe 0 -i "$LIST" \
  -vf "fps=$GIF_FPS,scale=$GIF_W:-2:flags=lanczos,split[a][b];[a]palettegen=max_colors=128:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle" \
  -loop 0 "$OUT/demo.gif"

ls -la "$OUT"/demo.mp4 "$OUT"/demo.gif | awk '{printf "%-28s %6.2f MB\n", $9, $5/1048576}'
ffprobe -v error -show_entries format=duration -of csv=p=0 "$OUT/demo.mp4" | awk '{printf "mp4 duration: %.1fs\n", $1}'
