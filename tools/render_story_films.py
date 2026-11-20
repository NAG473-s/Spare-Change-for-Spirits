#!/usr/bin/env python3
"""Render the three painted shots of each story ending with its German narration.

Run `python3 tools/render_story_films.py` to rebuild all six. To replace a
painting, pass `--sheet-dir DIR --only edwin` with DIR/edwin-painting.png
containing the opening and closing shots stacked vertically. Import a new
middle shot with `--detail-dir DIR --only edwin` from DIR/edwin.png. The
cropped production images live in art/endings. Requires Pillow, ffmpeg with
libmp3lame, and ffprobe on the production machine.
"""

import argparse
import json
import math
import subprocess
import tempfile
from pathlib import Path

from PIL import Image, ImageOps

ROOT = Path(__file__).resolve().parents[1]
GHOSTS = ("edwin", "ida", "lotte", "anselm", "mira", "kasimir")
WIDTH, HEIGHT = 960, 540
FPS = 8
BLEND_FRAMES = 4


def probe(path):
    result = subprocess.check_output([
        "ffprobe", "-v", "error", "-show_streams", "-show_format", "-of", "json", path,
    ])
    return json.loads(result)


def shots(sheet):
    width, height = sheet.size
    return [
        ImageOps.fit(
            sheet.crop((0, top, width, bottom)), (WIDTH, HEIGHT),
            method=Image.Resampling.LANCZOS,
        )
        for top, bottom in ((0, height // 2), (height // 2, height))
    ]


def import_sheet(src, dest):
    with Image.open(src) as original:
        first, second = shots(ImageOps.exif_transpose(original).convert("RGB"))
    sheet = Image.new("RGB", (WIDTH, HEIGHT * 2))
    sheet.paste(first, (0, 0))
    sheet.paste(second, (0, HEIGHT))
    dest.parent.mkdir(parents=True, exist_ok=True)
    sheet.save(dest, format="WEBP", quality=94, method=6)


def write_sequence(work, source, detail, total):
    with Image.open(source) as sheet:
        first, second = shots(sheet.convert("RGB"))
    with Image.open(detail) as middle:
        pictures = (first, middle.convert("RGB"), second)
    # The AVI muxer writes zero-byte hold frames for missing timestamps.
    # The last repeated JPEG fixes the duration even on a long closing shot.
    hold, remainder = divmod(total - 2 * (BLEND_FRAMES - 1), len(pictures))
    if hold < FPS:
        raise ValueError("Narration is too short for three one-second holds")
    tick = 0
    entries = []
    jpeg = {"quality": 92, "subsampling": 2, "optimize": True}
    for shot, picture in enumerate(pictures):
        filename = f"shot-{shot}.jpg"
        picture.save(work / filename, **jpeg)
        entries.append((tick, filename))
        tick += hold + int(shot < remainder)
        if shot == len(pictures) - 1:
            entries.append((total - 1, filename))
            break
        for step in range(1, BLEND_FRAMES):
            alpha = step / BLEND_FRAMES
            alpha = alpha * alpha * (3 - 2 * alpha)
            frame = f"blend-{shot}-{step}.jpg"
            Image.blend(picture, pictures[shot + 1], alpha).save(work / frame, **jpeg)
            entries.append((tick, frame))
            tick += 1
    playlist = ["ffconcat version 1.0"]
    for index, (tick, filename) in enumerate(entries):
        playlist.extend((f"file '{filename}'", f"option framerate {FPS}"))
        if index + 1 < len(entries):
            playlist.append(f"duration {(entries[index + 1][0] - tick) / FPS:.6f}")
    (work / "frames.ffconcat").write_text("\n".join(playlist) + "\n", encoding="ascii")


def render(ghost, sheet_dir, detail_dir):
    source = ROOT / "art" / "endings" / f"{ghost}.webp"
    detail = source.with_name(f"{ghost}_detail.webp")
    voice = ROOT / "assets" / "sounds" / "endings" / ghost / "de.ogg"
    output = ROOT / "assets" / "videos" / "endings" / f"{ghost}.avi"
    seconds = float(probe(voice)["format"]["duration"])
    total = math.ceil(seconds * FPS)
    if sheet_dir:
        import_sheet(sheet_dir / f"{ghost}-painting.png", source)
    if detail_dir:
        with Image.open(detail_dir / f"{ghost}.png") as image:
            framed = ImageOps.fit(ImageOps.exif_transpose(image).convert("RGB"), (WIDTH, HEIGHT),
                                  method=Image.Resampling.LANCZOS)
        detail.parent.mkdir(parents=True, exist_ok=True)
        framed.save(detail, format="WEBP", quality=94, method=6)
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=output.parent) as tmp:
        work = Path(tmp)
        write_sequence(work, source, detail, total)
        avi = work / "film.avi"
        subprocess.run([
            "ffmpeg", "-v", "error", "-nostdin", "-f", "concat", "-safe", "0",
            "-i", work / "frames.ffconcat", "-i", voice, "-map", "0:v", "-map", "1:a",
            "-c:v", "copy", "-c:a", "libmp3lame", "-b:a", "80k", "-ac", "1", "-ar", "24000",
            "-map_metadata", "-1", "-fflags", "+bitexact", "-avoid_negative_ts", "disabled", avi,
        ], check=True)
        video, audio = probe(avi)["streams"]
        if (
            video["codec_name"] != "mjpeg" or video["codec_tag_string"] != "MJPG"
            or (video["width"], video["height"]) != (WIDTH, HEIGHT)
            or int(video["nb_frames"]) != total or video["r_frame_rate"] != f"{FPS}/1"
            or audio["codec_name"] != "mp3" or audio["channels"] != 1
            or abs(float(audio["duration"]) - seconds) > 1 / FPS
        ):
            raise ValueError(f"Unexpected AVI stream format or duration for {ghost}")
        subprocess.run([
            "ffmpeg", "-v", "error", "-xerror", "-nostdin", "-i", avi, "-f", "null", "-",
        ], check=True)
        avi.replace(output)
    print(f"{ghost}: {total / FPS:.3f}s, {output.stat().st_size / 1048576:.2f} MiB", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sheet-dir", type=Path, help="import replacement two-panel PNGs")
    parser.add_argument("--detail-dir", type=Path, help="import replacement middle-shot PNGs")
    parser.add_argument("--only", choices=GHOSTS, nargs="+", default=GHOSTS)
    args = parser.parse_args()
    for ghost in args.only:
        render(ghost, args.sheet_dir, args.detail_dir)


if __name__ == "__main__":
    main()
