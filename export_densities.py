from PIL import Image, ImageFont, ImageDraw
import numpy as np

ASCII_CHARS = set("`1234567890-=~!@#$%^&*()_+qwertyuiop[]\\QWERTYUIOP{}|asdfghjkl;'ASDFGHJKL:\"zxcvbnm,./ZXCVBNM<>?")
if "'" in ASCII_CHARS:
    ASCII_CHARS.remove("'")

MIXED_CHARS = set("░▒▓█")

FONTFILE = "DejaVuSansMono.ttf"

def get_densities(chars: set[str]) -> list[tuple[str, float]]:
    font = ImageFont.truetype(FONTFILE, 10)

    width = round(font.getlength("a"))
    ascent, descent = font.getmetrics()
    height = ascent + descent

    densities: list[tuple[str, float]] = []

    for c in chars:
        img = Image.new("RGB", (width, height), (0, 0, 0))
        ImageDraw.Draw(img).text((0, ascent), c, font=font, anchor="ls", fill=(255, 255, 255))
        densities.append((c, np.array(img).mean().item() / 255 * 100))

    max_density = max(densities, key=lambda a: a[1])[1]

    densities = sorted(densities, key=lambda a: a[1])
    densities = [(c, d / max_density) for c, d in densities]

    return densities

def get_closest_char_idx(target: float, densities: list[tuple[str, float]]) -> int:
    return np.argmin(np.array(list(abs(target - d) for _, d in densities))).item()

def get_used_chars(chars: set[str]) -> list[tuple[str, float]]:
    d = get_densities(chars)
    used = [False] * len(d)
    for value in range(0, 255 * 3 + 1):
        used[get_closest_char_idx(value / 255 * 3, d)] = True
    for i in reversed(list(range(len(d)))):
        if not used[i]:
            d.pop(i)

    return d

if __name__ == "__main__":
    d = get_used_chars(ASCII_CHARS)
    print(f"const ASCII_CHARS: [(char, f64); {len(d)}] = {d};")

    d = get_used_chars(MIXED_CHARS)
    print(f"const MIXED_CHARS: [(char, f64); {len(d)}] = {d};")
