#!/bin/sh
# Renders a ten second sunset over the treeline to out/sunset.mp4. Extra
# options go to the video binary, e.g. ./video.sh --seed 3, and the
# environment sets size and file, e.g. WIDTH=1080 HEIGHT=1080 OUT=a.mp4
set -e
WIDTH=${WIDTH:-1280}
HEIGHT=${HEIGHT:-720}
OUT=${OUT:-out/sunset.mp4}
FPS=30
DURATION=10
cargo build --release
[ -f out/sky.bin ] || ./target/release/pixel-skies bake
./target/release/video --width $WIDTH --height $HEIGHT \
    --frames $((FPS * DURATION)) "$@" |
    ffmpeg -y -loglevel warning -f rawvideo -pix_fmt rgba \
        -s ${WIDTH}x$HEIGHT -r $FPS -i - \
        -c:v libx264 -pix_fmt yuv420p -crf 18 -movflags +faststart "$OUT"
