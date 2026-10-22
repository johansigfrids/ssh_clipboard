# Icon Assets (Internal)

## Purpose
Document how to create and regenerate the project icon assets used by the tray and (on Windows) the executable.

## Key Files
- `assets/icon.svg` (source of truth)
- `assets/icon.png` (tray icon)
- `assets/icon.ico` (Windows app icon)
- `build.rs` (embeds Windows icon)
- `desktop/src-tauri/tauri.conf.json`, `tauri.installed.conf.json` (desktop bundle icon inputs and installed overlay)

## Source Icon (SVG)
Keep the source icon as a simple, flat SVG at 1024×1024:
- Use a single artboard.
- Avoid thin strokes (icons are shown at 16–32 px).
- Keep contrast high and shapes simple.

Suggested workflow (pick one):
- **Figma/Sketch**: export as plain SVG.
- **Inkscape**: export as plain SVG.

Save as: `assets/icon.svg`.

## Generate PNG (tray icon)
Desktop icon configuration uses `assets/icon.png`; the desktop base configuration also lists `assets/icon.ico` for bundling. Keep these shared inputs synchronized.

Recommended size: **256×256** (scales down well).
If you update the SVG, regenerate the PNG:
- Inkscape (example):
  - Export PNG, width/height 256.
- ImageMagick (example):
  - `magick -background none -size 1024x1024 assets/icon.svg assets/icon.png`

## Generate Windows ICO
Windows uses `assets/icon.ico` embedded via `build.rs`.
Create a multi-size `.ico` (16/32/48/64/128/256).

ImageMagick example:
```
magick assets/icon.png -define icon:auto-resize=256,128,64,48,32,16 assets/icon.ico
```

Notes:
- If `assets/icon.ico` is missing, the build prints a warning and uses the default Windows icon.

## Optional macOS ICNS Export
Current Tauri bundling derives the Mac app icon from the base configuration's shared icon inputs. The optional manual export below is for external packaging tools; `assets/icon.icns` is generated output, not a required checked-in input.

Example workflow on macOS:
```
mkdir -p icon.iconset
sips -z 16 16     assets/icon.png --out icon.iconset/icon_16x16.png
sips -z 32 32     assets/icon.png --out icon.iconset/icon_16x16@2x.png
sips -z 32 32     assets/icon.png --out icon.iconset/icon_32x32.png
sips -z 64 64     assets/icon.png --out icon.iconset/icon_32x32@2x.png
sips -z 128 128   assets/icon.png --out icon.iconset/icon_128x128.png
sips -z 256 256   assets/icon.png --out icon.iconset/icon_128x128@2x.png
sips -z 256 256   assets/icon.png --out icon.iconset/icon_256x256.png
sips -z 512 512   assets/icon.png --out icon.iconset/icon_256x256@2x.png
sips -z 512 512   assets/icon.png --out icon.iconset/icon_512x512.png
sips -z 1024 1024 assets/icon.png --out icon.iconset/icon_512x512@2x.png
iconutil -c icns icon.iconset -o assets/icon.icns
```

Only external packaging that uses this export needs an explicit Info.plist `CFBundleIconFile` reference. Normal Tauri staging uses its existing configuration. Keep executable resource ownership aligned with [desktop development](desktop-development.md#layout-and-resource-guardrails).

## Update Triggers
- Changing the tray icon appearance.
- Packaging changes for Windows or macOS.

## Related Docs
- [docs/legacy/agent.md](history/0.4/legacy/agent.md)
- [docs/releasing.md](releasing.md)
