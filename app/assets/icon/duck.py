"""D.Duck icons (original character; not Ogu or any existing mascot).

- icon-1024.png: app icon. Navy rounded square filled edge to edge by a big round duck peeking over a yellow
  laptop, so it stays readable at Dock/taskbar sizes. Three colors: navy, white, yellow.
- tray-template.png / tray-template@2x.png: monochrome menu-bar icon (black on transparent; macOS tints it).

Usage: python3 -I duck.py <out_dir>
"""
import sys
from pathlib import Path

from PIL import Image, ImageDraw

out = Path(sys.argv[1])
NAVY, WHITE, YELLOW = (34, 40, 84, 255), (255, 255, 255, 255), (255, 200, 61, 255)
SS, S = 4, 1024


def k(*v):
    return [round(x * SS) for x in v]


# ---- app icon -------------------------------------------------------------------------------------------------
art = Image.new("RGBA", (S * SS, S * SS), NAVY)
d = ImageDraw.Draw(art)
d.ellipse(k(168, 214, 856, 902), fill=WHITE)                      # big head/body, wider than the lid
d.ellipse(k(356, 400, 420, 488), fill=NAVY)                        # eyes
d.ellipse(k(604, 400, 668, 488), fill=NAVY)
d.rounded_rectangle(k(424, 512, 600, 590), radius=39 * SS, fill=YELLOW)   # bill
d.rectangle(k(100, 650, 924, 924), fill=YELLOW)                    # laptop lid, runs off the bottom edge
d.ellipse(k(268, 612, 392, 700), fill=WHITE)                       # wings resting on the lid
d.ellipse(k(632, 612, 756, 700), fill=WHITE)
d.ellipse(k(484, 760, 540, 816), fill=NAVY)                        # lid camera dot

mask = Image.new("L", (S * SS, S * SS), 0)
ImageDraw.Draw(mask).rounded_rectangle(k(100, 100, 924, 924), radius=185 * SS, fill=255)
icon = Image.new("RGBA", (S * SS, S * SS), (0, 0, 0, 0))
icon.paste(art, (0, 0), mask)
icon.resize((S, S), Image.LANCZOS).save(out / "icon-1024.png")

# ---- tray template (22pt menu bar; drawn at 44px for @2x) -----------------------------------------------------
BLACK = (0, 0, 0, 255)
for size, name in [(22, "tray-template.png"), (44, "tray-template@2x.png")]:
    T = 16
    t = Image.new("RGBA", (size * T, size * T), (0, 0, 0, 0))
    g = ImageDraw.Draw(t)
    u = size * T / 22  # units of one point
    g.ellipse([3 * u, 2.5 * u, 19 * u, 18.5 * u], fill=BLACK)        # head
    g.ellipse([7.3 * u, 7.2 * u, 8.9 * u, 9.4 * u], fill=(0, 0, 0, 0))   # eyes cut out
    g.ellipse([13.1 * u, 7.2 * u, 14.7 * u, 9.4 * u], fill=(0, 0, 0, 0))
    g.rounded_rectangle([9 * u, 10.6 * u, 13 * u, 12.6 * u], radius=1 * u, fill=(0, 0, 0, 0))  # bill cut out
    g.rounded_rectangle([1.5 * u, 15 * u, 20.5 * u, 20 * u], radius=1.2 * u, fill=BLACK)    # laptop
    g.rectangle([3 * u, 14 * u, 19 * u, 15 * u], fill=(0, 0, 0, 0))   # gap between head and laptop
    t.resize((size, size), Image.LANCZOS).save(out / name)
print("ok")
