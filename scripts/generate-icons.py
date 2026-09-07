#!/usr/bin/env python3
"""Build Liusheng's original vector icon family and deterministic application assets.

The SVG paths in this file are the editable source of truth. No network access is used.
Run: python3 scripts/generate-icons.py [--check] [--contact-sheet PATH]
PNG / ICNS export uses CairoSVG and Pillow; runtime has no Python dependency.
"""
from __future__ import annotations

import argparse
import io
import json
from pathlib import Path
import struct
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "crates/liusheng/qml/assets"
# 24-unit grid, 1.75-unit strokes, round terminals, 2-unit optical safe area.
PATHS = {
    "album": '<rect x="3.5" y="3.5" width="17" height="17" rx="2"/><circle cx="12" cy="12" r="4.75"/><circle cx="12" cy="12" r=".9" fill="currentColor" stroke="none"/>',
    "artist": '<circle cx="12" cy="7.75" r="3.25"/><path d="M5 20v-1.25a7 6 0 0 1 14 0V20"/>',
    "music": '<path d="M9 17.25V5l11-2v12.25M9 8.5l11-2"/><ellipse cx="6.25" cy="17.75" rx="2.75" ry="2.25"/><ellipse cx="17.25" cy="15.75" rx="2.75" ry="2.25"/>',
    "playlist": '<path d="M4 5.5h16M4 10.5h16M4 15.5h9M4 20.5h9M18 15v6M15 18h6"/>',
    "queue": '<path d="M3.5 5.5h17M3.5 10.5h17M3.5 15.5h8M3.5 20.5h8"/><path d="m16 14 5 3.5-5 3.5z" fill="currentColor" stroke="none"/>',
    "play": '<path d="M7 5.1c0-.8.9-1.3 1.6-.85l11 6.9a1 1 0 0 1 0 1.7l-11 6.9c-.7.45-1.6-.05-1.6-.85z" fill="currentColor" stroke="none"/>',
    "pause": '<rect x="6.5" y="4.5" width="4" height="15" rx="1" fill="currentColor" stroke="none"/><rect x="13.5" y="4.5" width="4" height="15" rx="1" fill="currentColor" stroke="none"/>',
    "previous": '<path d="M5 5v14"/><path d="M18.5 5.7c0-.7-.75-1.1-1.3-.75L7 11.15a1 1 0 0 0 0 1.7l10.2 6.2c.55.35 1.3-.05 1.3-.75z" fill="currentColor" stroke="none"/>',
    "next": '<path d="M19 5v14"/><path d="M5.5 5.7c0-.7.75-1.1 1.3-.75L17 11.15a1 1 0 0 1 0 1.7l-10.2 6.2c-.55.35-1.3-.05-1.3-.75z" fill="currentColor" stroke="none"/>',
    "shuffle": '<path d="M3 6h2.5c5 0 7 12 12 12H21M17.5 14.5 21 18l-3.5 3.5M3 18h2.5c1.9 0 3.4-1.8 4.8-4M14 8c1.1-1.3 2.2-2 3.5-2H21M17.5 2.5 21 6l-3.5 3.5"/>',
    "repeat": '<path d="m16.5 3 4 4-4 4M20 7H7a4 4 0 0 0-4 4M7.5 21l-4-4 4-4M4 17h13a4 4 0 0 0 4-4"/>',
    "repeat-one": '<path d="m16.5 3 4 4-4 4M20 7H7a4 4 0 0 0-4 4M7.5 21l-4-4 4-4M4 17h13a4 4 0 0 0 4-4M10.5 10l2-1v5"/>',
    "search": '<circle cx="10.75" cy="10.75" r="6.25"/><path d="m15.5 15.5 4.5 4.5"/>',
    "filter": '<path d="M4 6h16M7 12h10M10 18h4"/>',
    "back": '<path d="m14.5 5-7 7 7 7"/>',
    "down": '<path d="m5 9 7 7 7-7"/>',
    "up": '<path d="m5 15 7-7 7 7"/>',
    "more": '<g fill="currentColor" stroke="none"><circle cx="5" cy="12" r="1.35"/><circle cx="12" cy="12" r="1.35"/><circle cx="19" cy="12" r="1.35"/></g>',
    "settings": '<path d="M4 6h7m5 0h4M4 12h2m5 0h9M4 18h9m5 0h2"/><circle cx="13.5" cy="6" r="2.5"/><circle cx="8.5" cy="12" r="2.5"/><circle cx="15.5" cy="18" r="2.5"/>',
    "folder": '<path d="M3 7V5.5A1.5 1.5 0 0 1 4.5 4H9l2 3h8.5A1.5 1.5 0 0 1 21 8.5v10a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 18.5V7Z"/>',
    "import": '<path d="M12 3v12m-4.5-4.5L12 15l4.5-4.5M4 15.5V20h16v-4.5"/>',
    "export": '<path d="M12 15V3m-4.5 4.5L12 3l4.5 4.5M4 15.5V20h16v-4.5"/>',
    "trash": '<path d="M4 6h16M9 6V3.5h6V6M6 6l1 14h10l1-14M10 10v6M14 10v6"/>',
    "lyrics": '<path d="M5 4h14a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H9l-5 3V6a2 2 0 0 1 1-2ZM8 8h9M8 12h6"/>',
    "output": '<path d="M7 17H5a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2h-2"/><path d="m12 13 5 7H7Z"/>',
    "volume": '<path d="m11 4-6 4H2.5v8H5l6 4ZM15 8a6 6 0 0 1 0 8M18 4.5a11 11 0 0 1 0 15"/>',
    "mute": '<path d="m10.5 4-6 4h-2v8h2l6 4ZM16 9l6 6m-6 0 6-6"/>',
    "close": '<path d="m6 6 12 12M6 18 18 6"/>',
    "check": '<path d="m4.5 12 5 5 10-11"/>',
    "plus": '<path d="M12 4.5v15M4.5 12h15"/>',
    "grip": '<g fill="currentColor" stroke="none"><circle cx="9" cy="5" r="1.2"/><circle cx="15" cy="5" r="1.2"/><circle cx="9" cy="12" r="1.2"/><circle cx="15" cy="12" r="1.2"/><circle cx="9" cy="19" r="1.2"/><circle cx="15" cy="19" r="1.2"/></g>',
    "refresh": '<path d="M20 4v6h-6M4 20v-6h6M5.1 8a7.5 7.5 0 0 1 12.1-3L20 10M4 14l2.8 5a7.5 7.5 0 0 0 12.1-3"/>',
    "clock": '<circle cx="12" cy="12" r="8.5"/><path d="M12 7v5l3.5 2"/>',
    "brand": '<path d="M15.4 6.3a8 8 0 1 0 2.3 8.9M12.7 9.1a4.5 4.5 0 1 0 1.4 5.7M17 3h4v7l-4.5 4.5"/><circle cx="10.5" cy="13" r="1" fill="currentColor" stroke="none"/>',
    # Preserve the previously shipped placeholder artwork glyph exactly.
    "disc": '<circle cx="12" cy="12" r="9"/><circle cx="12" cy="12" r="2"/><path d="M6 11a6 6 0 0 1 5-5M18 13a6 6 0 0 1-5 5"/>'
}


def svg(body: str, color: str = "#000000", size: int = 24, stroke: float = 1.75) -> str:
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" viewBox="0 0 24 24" '
            f'fill="none" stroke="{color}" stroke-width="{stroke}" stroke-linecap="round" stroke-linejoin="round">'
            + body.replace("currentColor", color) + '</svg>\n')


def app_svg(color: str = "#edece8", size: int = 512, *, themed: bool = False) -> str:
    """Reuse the in-app brand geometry at every size, with a transparent background.

    The desktop SVG exposes KDE's ColorScheme-Text hook; fixed-colour variants
    supply deterministic fallbacks for Qt, PNG exports and other desktops.
    """
    if not themed:
        return svg(PATHS["brand"], color, size)
    markup = svg(PATHS["brand"], "currentColor", size)
    opening, body = markup.split(">", 1)
    style = (f'<style id="current-color-scheme" type="text/css">'
             f'.ColorScheme-Text {{ color: {color}; }}'
             '</style>')
    return opening + ' class="ColorScheme-Text">' + style + body


def generated() -> dict[Path, bytes]:
    import cairosvg
    from PIL import Image, ImageDraw
    result = {ASSETS / "icons" / f"{name}.svg": svg(path, stroke=1.6 if name == "disc" else 1.75).encode()
              for name, path in PATHS.items()}
    manifest = {"family": "Liusheng Groove", "grid": 24, "stroke": 1.75, "icons": sorted(PATHS),
                "preserved_placeholder": "disc", "source": "scripts/generate-icons.py"}
    result[ASSETS / "icons/manifest.json"] = (json.dumps(manifest, ensure_ascii=False, indent=2) + '\n').encode()
    result[ASSETS / "app-icon/liusheng.svg"] = app_svg(themed=True).encode()
    for mode, color in [("light", "#242522"), ("dark", "#edece8")]:
        result[ASSETS / f"app-icon/tray-{mode}.svg"] = app_svg(color, 24).encode()
    result[ASSETS / "app-icon/liusheng-symbolic.svg"] = app_svg("#242522", 24, themed=True).encode()
    # Compatibility resource uses the same transparent mark as the sidebar.
    result[ASSETS / "tray.svg"] = app_svg(size=24).encode()
    sizes = (16, 24, 32, 48, 64, 128, 256, 512, 1024)
    pngs = {}
    for size in sizes:
        pngs[size] = cairosvg.svg2png(bytestring=app_svg().encode(), output_width=size, output_height=size)
        result[ASSETS / f"app-icon/{size}.png"] = pngs[size]
    # ICNS PNG chunks are portable and deterministic, including both Retina representations.
    chunks = []
    for code, size in [(b'icp4', 16), (b'icp5', 32), (b'icp6', 64), (b'ic07', 128),
                       (b'ic08', 256), (b'ic09', 512), (b'ic10', 1024)]:
        chunks.append(code + struct.pack('>I', len(pngs[size]) + 8) + pngs[size])
    payload = b''.join(chunks)
    result[ASSETS / "app-icon/Liusheng.icns"] = b'icns' + struct.pack('>I', len(payload) + 8) + payload
    # Fixed soft edge shadow for popups: scales as a nine-slice, no live blur/GPU dependency.
    from PIL import ImageFilter
    shadow = Image.new('RGBA', (64, 64))
    draw = ImageDraw.Draw(shadow)
    draw.rounded_rectangle((16, 16, 47, 47), 10, fill=(0, 0, 0, 32))
    shadow = shadow.filter(ImageFilter.GaussianBlur(7))
    buf = io.BytesIO(); shadow.save(buf, format='PNG')
    result[ASSETS / "popup-shadow.png"] = buf.getvalue()
    return result


def contact_sheet(path: Path) -> None:
    import cairosvg
    from PIL import Image, ImageDraw, ImageFont
    names = [name for name in PATHS if name != "disc"]
    sheet = Image.new('RGB', (1024, 768), '#faf9f6')
    d = ImageDraw.Draw(sheet)
    try:
        title = ImageFont.truetype('/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf', 28)
        label = ImageFont.truetype('/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf', 12)
    except OSError:
        title = label = ImageFont.load_default()
    d.text((40, 28), 'LIUSHENG / GROOVE', font=title, fill='#242522')
    d.text((40, 68), '24-unit grid  /  1.75-unit stroke  /  original vector assets', font=label, fill='#696a64')
    for i, name in enumerate(names):
        x, y = 40 + i % 8 * 120, 122 + i // 8 * 94
        icon = Image.open(io.BytesIO(cairosvg.svg2png(bytestring=svg(PATHS[name], '#242522').encode(), output_width=32, output_height=32)))
        sheet.paste(icon, (x + 28, y), icon)
        d.text((x, y + 43), name, font=label, fill='#696a64')
    for x, background, foreground in [(40, '#faf9f6', '#242522'), (168, '#18191b', '#edece8')]:
        d.rectangle((x, 626, x + 112, 738), fill=background)
        icon = Image.open(io.BytesIO(cairosvg.svg2png(bytestring=app_svg(foreground).encode(), output_width=112, output_height=112)))
        sheet.paste(icon, (x, 626), icon)
    d.text((304, 650), 'One brand mark / transparent / monochrome', font=label, fill='#242522')
    d.text((304, 678), '16 / 24 / 32 / 48 / 64 / 128 / 256 / 512 / 1024 px + ICNS', font=label, fill='#696a64')
    path.parent.mkdir(parents=True, exist_ok=True); sheet.save(path)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--contact-sheet', type=Path)
    args = parser.parse_args()
    files = generated()
    mismatches = []
    for path, data in files.items():
        if path.suffix == '.svg': ET.fromstring(data)
        if args.check:
            if not path.is_file() or path.read_bytes() != data: mismatches.append(str(path.relative_to(ROOT)))
        else:
            path.parent.mkdir(parents=True, exist_ok=True); path.write_bytes(data)
    if args.contact_sheet: contact_sheet(args.contact_sheet)
    if mismatches:
        print('Generated assets differ: ' + ', '.join(mismatches)); return 1
    print(f'{len(PATHS) - 1} redrawn UI icons; placeholder glyph retained; {len(files)} assets validated')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
