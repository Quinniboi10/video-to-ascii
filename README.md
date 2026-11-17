# Usage

Zoom out on your termianl as much as possbile. The further out you go, the higher the resolution.  
Higher resolutions require more terminal processing bandwidth. Go with something like [kitty](https://sw.kovidgoyal.net/kitty/) to get better performance if needed.

```bash
cargo r -r -- mixed video ./video.mp4
```
or
```bash
cargo r -r -- mixed image ./picture.jpg
```

## Modes
The first parameter is either `ascii`, `fullwidth-only`, or `mixed`.
- Ascii - Use only ASCII characters with luminance-based color approximation
- Fullwidth - Use only the Unicode full block character and raw RGB values
- Mixed - A gradient of unicode characters, using the same luminance color calculations

# Custom characters
To add custom characters, modify `export_densities.py`'s definition for `CHARS`

After modifications, run it and replace the definition for `CHARS` in `main.rs` with the script's output