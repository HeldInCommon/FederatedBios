#!/usr/bin/env python3
"""Make the quilt backgrounds: commons/static/quilt/quilt-1.svg … quilt-8.svg.

The app's background is a "quilt": the patterns from a deck of cards
(stripes, racetracks, shards, arrows, towers) laid edge to edge on a grid,
with grey felt between them. Each page uses one of eight quilts, chosen from
its title (see `quilt_for` in commons/src/web/layout.rs), so a page always
looks the same while different pages differ.

Two rules keep it calm enough to sit behind reading:
- tiles are dealt mostly around the edges: the chance of a tile falls toward
  the middle of the picture, which sits behind the page panel;
- a light veil of the felt colour is laid over everything.

The output is deterministic: the same seeds always make the same files, so
re-running this changes nothing unless the rules or seeds change. Standard
library only.

Usage: python3 scripts/make-quilts.py
"""
import math
import pathlib
import random

OUT = pathlib.Path(__file__).resolve().parent.parent / "commons" / "static" / "quilt"

# The size each quilt is drawn at. Pages show it fixed to the window and
# scaled to cover, so this is the shape of a typical desktop window.
WIDTH, HEIGHT = 1600, 1000
# One grid cell, in the drawing's pixels.
CELL = 140
# How likely a cell is to get a tile, before the edge weighting (0..1).
FILL = 0.5
# How much of the felt colour is laid over everything (0..1).
VEIL = 0.22
# The eight seeds: change one to re-deal that quilt.
SEEDS = [11, 23, 58, 104, 271, 333, 512, 909]

# Colours from the site's palette (commons/static/style.css).
FELT = "#e3e1e6"
STOCK = "#f9f9fa"
INK = "#111111"
OXBLOOD = "#451018"
GREY = "#a7a9ad"
PALETTE = ["#ff2d9b", "#1f5cff", "#12b37d", "#8b3fd9"]
# How often a tile is oxblood (a pop) or ink rather than pale stock.
OXBLOOD_CHANCE = 0.05
INK_CHANCE = 0.07


class Noise:
    """Smooth 2-D value noise from a fixed seed, for slow variation across
    the grid (where tiles cluster, which way arrows point)."""

    def __init__(self, seed):
        rng = random.Random(seed)
        self.table = [rng.random() for _ in range(512)]

    def _corner(self, x, y):
        return self.table[(x * 73 + y * 151) % 512]

    def __call__(self, x, y):
        xi, yi = math.floor(x), math.floor(y)
        xf, yf = x - xi, y - yi
        # Smoothstep the fractions so the field has no visible creases.
        u, v = xf * xf * (3 - 2 * xf), yf * yf * (3 - 2 * yf)
        top = self._corner(xi, yi) * (1 - u) + self._corner(xi + 1, yi) * u
        bottom = self._corner(xi, yi + 1) * (1 - u) + self._corner(xi + 1, yi + 1) * u
        return top * (1 - v) + bottom * v


def smoothstep(edge0, edge1, x):
    """0 below edge0, 1 above edge1, a smooth S-curve between."""
    t = min(1.0, max(0.0, (x - edge0) / (edge1 - edge0)))
    return t * t * (3 - 2 * t)


def edge_weight(cx, cy):
    """How strongly a tile centred at (cx, cy) belongs: near 0 in the middle
    of the picture, 1 towards the edges. Uses the larger of the horizontal
    and vertical distances, so the calm middle is a soft rectangle."""
    dx = abs(cx - WIDTH / 2) / (WIDTH / 2)
    dy = abs(cy - HEIGHT / 2) / (HEIGHT / 2)
    return 0.08 + 0.92 * smoothstep(0.35, 0.9, max(dx, dy))


def polygon(points, fill):
    """An SVG polygon from (x, y) pairs."""
    pts = " ".join(f"{x:.1f},{y:.1f}" for x, y in points)
    return f'<polygon points="{pts}" fill="{fill}"/>'


def motif(kind, w, h, inks, rng):
    """One tile's pattern, drawn around (0, 0) in a w-by-h box."""
    out = []
    if kind == "stripes":
        # One or two families of bands crossing at different angles.
        for family in range(2 if rng.random() < 0.6 else 1):
            angle = math.degrees(rng.uniform(-0.7, 0.7) + family * rng.uniform(0.9, 1.6))
            band = rng.uniform(9, 16)
            out.append(f'<g transform="rotate({angle:.1f})">')
            for k in range(-9, 10):
                if rng.random() < 0.35:
                    continue
                colour = inks[(k + 9 + family) % len(inks)]
                out.append(f'<rect x="{-h:.0f}" y="{k * band * 1.7:.1f}" width="{2 * h:.0f}" height="{band:.1f}" fill="{colour}"/>')
            out.append("</g>")
    elif kind == "arrow":
        aw, ah = w * 0.7, h * 0.72
        neck = -ah / 2 + aw * 0.55
        points = [(0, -ah / 2), (aw / 2, neck), (aw * 0.17, neck), (aw * 0.2, ah / 2),
                  (-aw * 0.2, ah / 2), (-aw * 0.17, neck), (-aw / 2, neck)]
        out.append(polygon(points, GREY if rng.random() < 0.25 else inks[0]))
    elif kind == "racetrack":
        # Concentric rounded outlines, alternating two inks.
        stroke = rng.uniform(5, 8)
        ww, hh = w * 0.82, h * 0.82
        ring = 0
        while ww > stroke * 2 and hh > stroke * 2:
            out.append(f'<rect x="{-ww / 2:.1f}" y="{-hh / 2:.1f}" width="{ww:.1f}" height="{hh:.1f}" '
                       f'rx="{min(ww, hh) / 2:.1f}" fill="none" stroke="{inks[ring % 2]}" stroke-width="{stroke:.1f}"/>')
            ww -= stroke * 2.6
            hh -= stroke * 2.6
            ring += 1
    elif kind == "shards":
        # A fan of triangles from an off-centre point.
        cx, cy = rng.uniform(-w * 0.2, w * 0.2), rng.uniform(-h * 0.2, h * 0.2)
        count = rng.randint(8, 12)
        reach = max(w, h)
        a0 = rng.uniform(0, 2 * math.pi)
        for i in range(count):
            a1 = a0 + 2 * math.pi / count * rng.uniform(0.7, 1.3)
            colour = GREY if rng.random() < 0.2 else inks[i % len(inks)]
            out.append(polygon([(cx, cy), (cx + math.cos(a0) * reach, cy + math.sin(a0) * reach),
                                (cx + math.cos(a1) * reach, cy + math.sin(a1) * reach)], colour))
            a0 = a1
    elif kind == "tower":
        # Stacked trapezoids, narrowing upwards, slightly skewed.
        layers = rng.randint(4, 6)
        layer_h = h * 0.78 / layers
        for i in range(layers):
            y = h * 0.39 - (i + 1) * layer_h
            base = w * 0.86 - (w * 0.55) * i / max(1, layers - 1)
            skew = rng.uniform(-6, 6)
            colour = GREY if i % 3 == 2 else inks[i % len(inks)]
            out.append(polygon([(-base / 2 + skew, y + layer_h), (base / 2 + skew, y + layer_h),
                                (base * 0.38, y), (-base * 0.38, y)], colour))
    return "".join(out)


def quilt(seed):
    """The SVG text of one quilt."""
    rng = random.Random(seed)
    noise = Noise(seed)
    cols, rows = math.ceil(WIDTH / CELL), math.ceil(HEIGHT / CELL)
    taken = [[False] * cols for _ in range(rows)]
    parts = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{HEIGHT}" '
             f'viewBox="0 0 {WIDTH} {HEIGHT}" preserveAspectRatio="xMidYMid slice">',
             f'<rect width="{WIDTH}" height="{HEIGHT}" fill="{FELT}"/>']
    tile = 0
    for r in range(rows):
        for c in range(cols):
            if taken[r][c]:
                continue
            taken[r][c] = True
            # Clusters from the noise field, thinned towards the middle.
            busy = noise(c * 0.35 + 9, r * 0.35 + 9)
            centre_x, centre_y = (c + 0.5) * CELL, (r + 0.5) * CELL
            if rng.random() > FILL * (0.4 + busy) * edge_weight(centre_x, centre_y):
                continue
            # Now and then a tile spans two cells, or two by two.
            span_w = span_h = 1
            roll = rng.random()
            room_right = c + 1 < cols and not taken[r][c + 1]
            room_down = r + 1 < rows and not taken[r + 1][c]
            if roll < 0.1 and room_right and room_down and not taken[r + 1][c + 1]:
                span_w = span_h = 2
            elif roll < 0.28 and room_right:
                span_w = 2
            for dr in range(span_h):
                for dc in range(span_w):
                    taken[r + dr][c + dc] = True
            w, h = span_w * CELL, span_h * CELL
            kind = rng.choice(["stripes"] * 3 + ["arrow"] * 2 + ["racetrack"] * 3 + ["shards"] * 2 + ["tower"] * 2)
            if rng.random() < OXBLOOD_CHANCE:
                stock = OXBLOOD
            elif rng.random() < INK_CHANCE:
                stock = INK
            else:
                stock = STOCK
            inks = rng.sample(PALETTE, 3)
            # Arrows follow the slow field in 45-degree steps, so they flow.
            turn = round((noise(c * 0.15, r * 0.15) - 0.5) * 8) * 45 if kind == "arrow" else 0
            tile += 1
            x0, y0 = c * CELL, r * CELL
            parts.append(f'<svg x="{x0}" y="{y0}" width="{w}" height="{h}" viewBox="{-w / 2} {-h / 2} {w} {h}" overflow="hidden">')
            parts.append(f'<rect x="{-w / 2}" y="{-h / 2}" width="{w}" height="{h}" fill="{stock}"/>')
            parts.append(f'<g transform="rotate({turn})">{motif(kind, w, h, inks, random.Random(seed * 31 + tile))}</g></svg>')
    parts.append(f'<rect width="{WIDTH}" height="{HEIGHT}" fill="{FELT}" fill-opacity="{VEIL}"/>')
    parts.append("</svg>\n")
    return "".join(parts)


def main():
    """Write the eight quilts, replacing any already there."""
    OUT.mkdir(parents=True, exist_ok=True)
    for number, seed in enumerate(SEEDS, start=1):
        path = OUT / f"quilt-{number}.svg"
        path.write_text(quilt(seed))
        print(f"{path.relative_to(OUT.parent.parent.parent)}: seed {seed}, {path.stat().st_size // 1024} KB")


if __name__ == "__main__":
    main()
