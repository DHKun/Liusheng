"""Regressions for a single, transparent Liusheng brand mark across all exports.

Run: python3 -m unittest discover -s tests -p 'test_icon_identity.py' -v
Requires the same CairoSVG/Pillow tooling as scripts/generate-icons.py.
"""
from __future__ import annotations

import importlib.util
import io
from pathlib import Path
import struct
import unittest
import xml.etree.ElementTree as ET

import cairosvg
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "crates/liusheng/qml/assets"
spec = importlib.util.spec_from_file_location("generate_icons", ROOT / "scripts/generate-icons.py")
assert spec is not None and spec.loader is not None
icons = importlib.util.module_from_spec(spec)
spec.loader.exec_module(icons)


def shape(data: bytes) -> tuple:
    root = ET.fromstring(data)
    return (
        root.attrib["viewBox"], root.attrib["stroke-width"],
        root.attrib["stroke-linecap"], root.attrib["stroke-linejoin"],
        tuple((child.tag.rsplit("}", 1)[-1], tuple(sorted(
            (key, value) for key, value in child.attrib.items() if key not in ("fill", "stroke", "class")
        ))) for child in root if child.tag.rsplit("}", 1)[-1] != "style"),
    )


def render(data: bytes, size: int) -> Image.Image:
    return Image.open(io.BytesIO(cairosvg.svg2png(
        bytestring=data, output_width=size, output_height=size
    ))).convert("RGBA")


class UnifiedBrandTests(unittest.TestCase):
    def test_tray_uses_embedded_pixels_instead_of_a_theme_name(self):
        source = (ROOT / "crates/liusheng/qml/Main.qml").read_text()
        block = source.split("Platform.SystemTrayIcon {", 1)[1].split("tooltip:", 1)[0]
        self.assertIn('icon.name: ""', block)
        self.assertIn('assets/app-icon/tray-light.svg', block)
        self.assertIn('assets/app-icon/tray-dark.svg', block)
        self.assertNotIn('icon.name: "io.github', block)

    def test_every_svg_reuses_the_exact_sidebar_geometry(self):
        reference = shape((ASSETS / "icons/brand.svg").read_bytes())
        candidates = [ASSETS / "tray.svg", *sorted((ASSETS / "app-icon").glob("*.svg"))]
        self.assertEqual(len(candidates), 5)
        for path in candidates:
            with self.subTest(asset=path.name):
                self.assertEqual(shape(path.read_bytes()), reference)
                self.assertEqual(ET.fromstring(path.read_bytes()).attrib["fill"], "none")

    def test_pngs_keep_the_brand_alpha_at_every_size(self):
        source = (ASSETS / "icons/brand.svg").read_bytes()
        for size in (16, 24, 32, 48, 64, 128, 256, 512, 1024):
            with self.subTest(size=size):
                image = Image.open(ASSETS / f"app-icon/{size}.png").convert("RGBA")
                self.assertEqual(image.size, (size, size))
                self.assertEqual(image.getchannel("A").tobytes(), render(source, size).getchannel("A").tobytes())
                self.assertEqual(image.getpixel((0, 0))[3], 0)
                self.assertEqual(image.getpixel((size - 1, size - 1))[3], 0)
                self.assertLess(sum(a > 0 for a in image.getchannel("A").getdata()), size * size * 0.55)

    def test_light_dark_and_themed_svg_keep_the_same_alpha(self):
        reference = render((ASSETS / "icons/brand.svg").read_bytes(), 64).getchannel("A").tobytes()
        for path in (ASSETS / "app-icon").glob("*.svg"):
            with self.subTest(asset=path.name):
                image = render(path.read_bytes(), 64)
                self.assertEqual(image.getchannel("A").tobytes(), reference)
        for name, rgb in (("tray-dark", (237, 236, 232)), ("tray-light", (36, 37, 34))):
            image = render((ASSETS / f"app-icon/{name}.svg").read_bytes(), 64)
            self.assertTrue(any(pixel == (*rgb, 255) for pixel in image.getdata()))

    def test_icns_contains_the_same_png_exports(self):
        data = (ASSETS / "app-icon/Liusheng.icns").read_bytes()
        self.assertEqual(data[:4], b"icns")
        self.assertEqual(struct.unpack(">I", data[4:8])[0], len(data))
        position = 8
        sizes = {b"icp4": 16, b"icp5": 32, b"icp6": 64, b"ic07": 128,
                 b"ic08": 256, b"ic09": 512, b"ic10": 1024}
        seen = set()
        while position < len(data):
            code = data[position:position + 4]
            length = struct.unpack(">I", data[position + 4:position + 8])[0]
            self.assertGreater(length, 8)
            self.assertLessEqual(position + length, len(data))
            self.assertEqual(data[position + 8:position + length],
                             (ASSETS / f"app-icon/{sizes[code]}.png").read_bytes())
            seen.add(code)
            position += length
        self.assertEqual(seen, set(sizes))
        self.assertEqual(position, len(data))

    def test_symbolic_desktop_resource_exposes_theme_colour(self):
        for filename in ("liusheng.svg", "liusheng-symbolic.svg"):
            root = ET.fromstring((ASSETS / "app-icon" / filename).read_bytes())
            self.assertEqual(root.attrib["class"], "ColorScheme-Text")
            self.assertEqual(root.attrib["stroke"], "currentColor")
            styles = root.findall("{http://www.w3.org/2000/svg}style")
            self.assertEqual(len(styles), 1)
            self.assertEqual(styles[0].attrib["id"], "current-color-scheme")


if __name__ == "__main__":
    unittest.main()
