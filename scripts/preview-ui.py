#!/usr/bin/env python3
"""Capture the actual application with an isolated, fictional music collection.

Usage: python3 scripts/preview-ui.py /path/to/liusheng --output target/qa/design
Optional test-only dependency: Pillow. All artwork is generated locally as fixture data.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
from pathlib import Path
import shutil
import sqlite3
import tempfile


def artwork(path: Path, number: int, title: str) -> None:
    from PIL import Image, ImageDraw, ImageFont
    palettes = [
        ("#e5d9c4", "#b16744", "#282824"), ("#233642", "#c0d1ca", "#dca278"),
        ("#e6e8df", "#617866", "#b7beab"), ("#cab5a1", "#594a40", "#eee2d5"),
        ("#c46f3f", "#f6d5a5", "#343a32"), ("#d0d3b7", "#73806a", "#353f36"),
        ("#e2ddd1", "#6f7a7b", "#ba7d65"), ("#9ca9ad", "#4e6066", "#e0e3de"),
        ("#263b35", "#87a691", "#c5c9a4"), ("#e0bf64", "#9b5643", "#f4e9c6"),
        ("#4b6c77", "#cdd4c6", "#263f4d"), ("#e3dbd4", "#474d58", "#ae6c5c")
    ]
    bg, fg, detail = palettes[number % len(palettes)]
    image = Image.new("RGB", (640, 640), bg)
    draw = ImageDraw.Draw(image)
    kind = number % 4
    if kind == 0:
        draw.ellipse((110, 100, 520, 510), fill=fg)
        draw.rectangle((0, 330, 640, 345), fill=detail)
        draw.ellipse((268, 268, 372, 372), fill=bg)
    elif kind == 1:
        for n in range(7):
            y = 85 + n * 62
            draw.line((58, y, 583, y + n * 10), fill=fg, width=2 + n * 3)
        draw.ellipse((410, 75, 495, 160), fill=detail)
    elif kind == 2:
        draw.polygon([(0, 470), (210, 170), (420, 510)], fill=fg)
        draw.polygon([(180, 560), (430, 180), (660, 570)], fill=detail)
        draw.rectangle((0, 548, 640, 640), fill=bg)
    else:
        for n in range(10):
            x = 45 + n * 55
            draw.rectangle((x, 75 + (n % 4) * 30, x + 17, 505), fill=fg)
        draw.rectangle((300, 160, 590, 190), fill=detail)
    font_path = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"
    try:
        small = ImageFont.truetype(font_path, 16)
        large = ImageFont.truetype(font_path, 23)
    except OSError:
        small = large = ImageFont.load_default()
    draw.text((32, 30), "LIUSHENG / STUDIES", font=small, fill=detail)
    draw.text((32, 582), title.upper(), font=large, fill=detail)
    image.save(path)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--long-labels", action="store_true", help="Exercise long Chinese, Japanese and English metadata")
    args = parser.parse_args()
    spec = importlib.util.spec_from_file_location("ui_checks", Path(__file__).with_name("check-ui.py"))
    qa = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(qa)
    binary = str(args.binary.resolve(strict=True))
    args.output.mkdir(parents=True, exist_ok=True)
    albums = [
        ("Soft Geometry", "North Field"), ("Blue Hours", "North Field"),
        ("Quiet Orbit", "Mira Vale"), ("Between Seasons", "Mira Vale"),
        ("Window Seat", "New Coast"), ("A Place to Begin", "New Coast"),
        ("City Walks", "白日集"), ("After the Rain", "白日集"),
        ("Distant Mountains", "林间"), ("Sunday Sketches", "林间"),
        ("Night Flight", "Low Tide"), ("Still / Moving", "Low Tide")
    ]
    with tempfile.TemporaryDirectory(prefix="liusheng-preview-") as directory:
        root = Path(directory)
        env = qa.fixture(root)
        env["LIUSHENG_ALLOW_MULTIPLE"] = "1"
        settings = root / "config/liusheng/settings.json"
        preferences = json.loads(settings.read_text())
        preferences["appearance"] = "light"
        preferences["reduced_motion"] = True
        settings.write_text(json.dumps(preferences))
        qa.run(["dbus-run-session", "--", binary, "--startup-benchmark"], env)
        wav = (root / "music/Track 1.wav").read_bytes()
        for item in (root / "music").iterdir():
            item.unlink()  # Only the files created by fixture() in this temporary directory.
        conn = sqlite3.connect(root / "data/liusheng/library.db")
        conn.execute("DELETE FROM tracks")
        paths = []
        for number, (album, artist) in enumerate(albums):
            folder = root / "music" / f"Album-{number:02d}"
            folder.mkdir()
            artwork(folder / "cover.png", number, album)
            if args.long_labels:
                album = f"{number:02d} · " + ("The Quiet Space Between Every Unfinished Conversation / " * 3 if number % 3 == 0
                    else "走过漫长的街道仍然想把每一段音乐留在身边 · " * 5 if number % 3 == 1
                    else "いつかまたこの場所で音楽を聴きながら会いましょう · " * 4)
                artist = ["A very long artist name & guest collaborators", "林间与白日集联合演奏特别版", "夜明けのアンサンブルと仲間たち"][number % 3]
            for index, title in enumerate(["We Were Here", "慢慢，走过这条街", "The Space Between", "A Quiet Return"]):
                if args.long_labels:
                    title = ["A very long title / 很长的中文曲名 / とても長い日本語の曲名 / ", "未完待续的旋律与故事", "The Space Between", "<b>A Quiet Return</b>"][index] * 4
                path = folder / f"{index + 1:02d}.wav"
                path.write_bytes(wav)
                stat = path.stat()
                conn.execute(
                    "INSERT INTO tracks(path,mtime,file_size,title,artist,album,album_artist,track_no,disc_no,year,"
                    "duration_ms,sample_rate,bit_depth,channels,title_search,artist_search,album_search) "
                    "VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
                    (str(path), stat.st_mtime_ns, stat.st_size, title, artist, album, artist, index + 1, 1,
                     2020 + number % 5, 2000, 48000, 16, 2, title.lower(), artist.lower(), album.lower()))
                paths.append(str(path))
            (folder / "01.lrc").write_text("[00:00.00]让风经过窗前\n[00:00.40]把这一刻留给音乐\n[00:01.00]静静听见\n[00:01.60]生活的另一面\n", encoding="utf-8")
        if args.long_labels:
            (root / "music/Album-00/01.lrc").write_text("[00:00.00]沿着这条很长很长的街道走下去，直到听见每一段音乐的回响\n[00:00.40]いつかまたこの場所で音楽を聴きながら会いましょう\n[00:01.00]The quiet space between every unfinished conversation\n", encoding="utf-8")
        for name in ["晨间留白", "夜の散歩", "A Quiet Weekend" if not args.long_labels else "A Quiet Weekend / 晨间留白 / 夜の散歩 " * 6]:
            row = conn.execute("INSERT INTO playlists(name) VALUES(?)", (name,)).lastrowid
            conn.executemany("INSERT INTO playlist_entries(playlist_id,position,path) VALUES(?,?,?)", [(row, i, path) for i, path in enumerate(paths[:8])])
        conn.commit()
        conn.close()
        session = {"version": 1, "queue": paths[:8], "current_index": 0, "position_ms": 700, "page": "albums", "width": 1280, "height": 800}
        (root / "data/liusheng/session.json").write_text(json.dumps(session))
        log = qa.run(["dbus-run-session", "--", binary, "--ui-test"], env)
        qa.inspect_log(log)
        (args.output / "preview.log").write_text(log)
        shutil.copytree(root / "screens", args.output / "screens", dirs_exist_ok=True)
    (args.output / "README.txt").write_text(
        "Actual application screenshots, rendered with generated fictional album artwork and a temporary test library.\n"
        "No production user library or settings were used. Album titles and performers are illustrative fixtures.\n")
    print(f"Captured application previews in {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
