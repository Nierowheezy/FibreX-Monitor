# App icons

The app icon is the FibreX mark: three chevrons fanning out from a point, drawn
in ember (`#ff5a1c`) on the dark bento canvas (`#09090b`). It is the same mark and
the same two tokens the topbar wordmark uses, so the Dock, Finder, and the
topbar cannot drift apart.

## Files

- `icon-source.svg` - the hand-authored source. Edit this, never the PNGs.
- `icon.png` - the 1024x1024 master, rendered from the source.
- `32x32.png`, `128x128.png`, `128x128@2x.png` - window and list sizes.
- `icon.icns` - the macOS bundle icon.
- `icon.ico` - the Windows bundle icon.
- `64x64.png`, `Square*Logo.png`, `StoreLogo.png` - extra sizes the Tauri icon
  generator emits. They are committed so a rebuild is reproducible.

The menu-bar glyph is separate: `src-tauri/assets/tray-icon.png`. It is the same
mark as the app icon, reduced to a macOS template image (black with alpha) so
macOS can tint it for a light or dark menu bar. It is derived from `icon.png`
rather than hand-drawn, so the menu bar cannot drift from the app icon either.

## The menu-bar glyph

`tray-icon.png` is a 64x64 template: black pixels, with the alpha channel carrying
the mark. Two things matter for a template image: the colour must be black (macOS
ignores it and tints by alpha), and the artwork must sit inside the visible area
with a little padding rather than touching the edges.

It is produced from the master icon with Pillow, which the regeneration machine
has. The mark occupies the 552x552 group starting at (236, 236) in the 1024
master; the red channel is the cleanest coverage signal because the mark is
`#ff5a1c` (255 red) on a `#09090b` tile (9 red):

```py
from PIL import Image

source = Image.open("src-tauri/icons/icon.png").convert("RGB")
mark = source.crop((236, 236, 788, 788))

alpha = Image.new("L", mark.size)
for y in range(mark.height):
    for x in range(mark.width):
        coverage = (mark.getpixel((x, y))[0] - 9) / (255 - 9)
        alpha.putpixel((x, y), max(0, min(255, round(coverage * 255))))

art = alpha.crop(alpha.point(lambda value: 255 if value > 4 else 0).getbbox())
side = max(art.size)
padding = round(side * 0.04)
canvas = Image.new("L", (side + padding * 2, side + padding * 2), 0)
canvas.paste(art, ((side - art.width) // 2 + padding, (side - art.height) // 2 + padding))
canvas = canvas.resize((64, 64), Image.LANCZOS)

black = Image.new("L", canvas.size, 0)
Image.merge("RGBA", (black, black, black, canvas)).save("src-tauri/assets/tray-icon.png")
```

The 30% and 55% opacity of the fan's outer chevrons survives as partial alpha, so
the menu-bar mark keeps the same weighting as the app icon. At 64x64 the image is
downsampled by macOS to the menu bar's 18 pt height, which keeps the edges clean
on a Retina display.

## Regenerating

There is no SVG rasteriser installed on this machine, so the 1024 master is
produced by drawing the SVG on a canvas in headless Chrome. The script is kept
out of the repository because it is a one-off build tool that talks to Chrome
over the DevTools protocol.

The steps, in order:

1. Render the 1024x1024 master from `icon-source.svg` into `icon.png`.
   The render is checked before it is written: a wrong tile colour, an opaque
   corner, or a missing mark all stop the write.
2. Generate the platform sizes from that master:

   ```sh
   npx tauri icon src-tauri/icons/icon.png -o src-tauri/icons
   ```

3. Delete the platform folders this desktop-only app does not target, which the
   generator creates anyway:

   ```sh
   rm -rf src-tauri/icons/android src-tauri/icons/ios
   ```

4. Re-render `icon.png` from step 1, because step 2 overwrites it with a 512x512
   copy. Skipping this silently downgrades the master that the next regeneration
   starts from.

5. Confirm the sizes are right:

   ```sh
   for f in icon.png 128x128.png 128x128@2x.png 32x32.png; do
     echo -n "$f "; sips -g pixelWidth -g pixelHeight "src-tauri/icons/$f" | grep pixel | tr '\n' ' '
     echo
   done
   ```

   Expected: `icon.png` 1024, `128x128@2x.png` 256, `128x128.png` 128,
   `32x32.png` 32.

6. Rebuild the bundles and confirm Finder and the Dock show the new icon:

   ```sh
   npm run tauri build
   ```

## Why the master wrote over itself once

`npx tauri icon <input> -o <same directory>` writes a 512x512 `icon.png` to the
output directory. When the input and the output are the same file, the 1024
master is replaced by a 512 copy. Step 4 above exists to undo that. Keeping the
master at 1024 matters because every generated size is downscaled from it.
