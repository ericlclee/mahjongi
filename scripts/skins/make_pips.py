"""Draws the Pips tile skin as SVG files into web/skins/pips/tiles/.

Run from the repository root:  python3 scripts/skins/make_pips.py
"""
import os

OUT = os.path.join("web", "skins", "pips", "tiles")
W, H = 60, 84
RED, BLUE, GREEN, INK = "#c0392b", "#1f5fa8", "#2e7d32", "#222"
FONT = "font-family='-apple-system, system-ui, sans-serif' font-weight='800' text-anchor='middle'"

# Pip centres for 1-9, in the usual arrangements.
SPOTS = {
    1: [(30, 42)],
    2: [(30, 24), (30, 60)],
    3: [(16, 18), (30, 42), (44, 66)],
    4: [(18, 24), (42, 24), (18, 60), (42, 60)],
    5: [(18, 22), (42, 22), (30, 42), (18, 62), (42, 62)],
    6: [(18, 18), (42, 18), (18, 42), (42, 42), (18, 66), (42, 66)],
    7: [(14, 14), (30, 24), (46, 34), (18, 52), (42, 52), (18, 70), (42, 70)],
    8: [(18, 14), (42, 14), (18, 33), (42, 33), (18, 52), (42, 52), (18, 71), (42, 71)],
    9: [(14, 18), (30, 18), (46, 18), (14, 42), (30, 42), (46, 42), (14, 66), (30, 66), (46, 66)],
}
NUMERALS = "一二三四五六七八九"


def frame(body, fill="#fbf8ef"):
    return (f"<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 {W} {H}'>"
            f"<rect x='0' y='0' width='{W}' height='{H}' rx='6' fill='{fill}'/>{body}</svg>")


def dots(n):
    r = 15 if n == 1 else 9 if n <= 4 else 7.5 if n <= 6 else 6.5
    colours = [BLUE, RED, GREEN]
    out = []
    for k, (x, y) in enumerate(SPOTS[n]):
        c = RED if n == 1 else colours[k % 3] if n >= 7 else BLUE if k % 2 == 0 else GREEN
        out.append(f"<circle cx='{x}' cy='{y}' r='{r}' fill='none' stroke='{c}' stroke-width='{r * 0.35:.1f}'/>"
                   f"<circle cx='{x}' cy='{y}' r='{r * 0.35:.1f}' fill='{c}'/>")
    return "".join(out)


def bamboo(n):
    if n == 1:  # a single tall stick, as the 1 is often a bird instead
        return (f"<rect x='24' y='12' width='12' height='60' rx='6' fill='{GREEN}'/>"
                f"<rect x='24' y='38' width='12' height='4' fill='#fbf8ef'/><circle cx='30' cy='12' r='6' fill='{RED}'/>")
    h = 30 if n <= 3 else 18 if n <= 6 else 15
    w = 8 if n <= 6 else 6
    out = []
    for k, (x, y) in enumerate(SPOTS[n]):
        c = RED if (n in (5, 7, 9) and k == len(SPOTS[n]) // 2) else GREEN
        out.append(f"<rect x='{x - w / 2}' y='{y - h / 2}' width='{w}' height='{h}' rx='{w / 2}' fill='{c}'/>"
                   f"<rect x='{x - w / 2}' y='{y - 1}' width='{w}' height='2' fill='#fbf8ef'/>")
    return "".join(out)


def characters(n):
    return (f"<text x='30' y='38' font-size='28' fill='{INK}' {FONT}>{NUMERALS[n - 1]}</text>"
            f"<text x='30' y='72' font-size='26' fill='{RED}' {FONT}>萬</text>")


def honour(text, colour):
    return f"<text x='30' y='56' font-size='40' fill='{colour}' {FONT}>{text}</text>"


def main():
    os.makedirs(OUT, exist_ok=True)
    tiles = {}
    for n in range(1, 10):
        tiles[f"{n}m"] = characters(n)
        tiles[f"{n}p"] = dots(n)
        tiles[f"{n}s"] = bamboo(n)
    for i, ch in enumerate("東南西北"):
        tiles[f"{i + 1}z"] = honour(ch, INK)
    tiles["5z"] = f"<rect x='12' y='14' width='36' height='56' rx='3' fill='none' stroke='{BLUE}' stroke-width='4'/>"
    tiles["6z"] = honour("發", GREEN)
    tiles["7z"] = honour("中", RED)
    for code, body in tiles.items():
        with open(os.path.join(OUT, f"{code}.svg"), "w", encoding="utf-8") as f:
            f.write(frame(body))
    back = "".join(f"<circle cx='{x}' cy='{y}' r='3' fill='#2c9a6c'/>" for x in range(8, 60, 11) for y in range(8, 84, 11))
    with open(os.path.join(OUT, "back.svg"), "w", encoding="utf-8") as f:
        f.write(frame(back, fill="#1b7a55"))
    print(f"wrote {len(tiles) + 1} files to {OUT}")


if __name__ == "__main__":
    main()
