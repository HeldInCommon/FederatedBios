# Fonts for the app's own screens

The setup window works offline, so it can't load Google Fonts the way the
site does (decision record 0013). These are the same two families, shipped
inside the app:

| File | Family | Licence |
|---|---|---|
| `SpaceGrotesk.woff2` | Space Grotesk, weights 400 to 700 (variable) | SIL Open Font Licence 1.1, `SpaceGrotesk-OFL.txt` |
| `SpaceMono-Regular.woff2`, `SpaceMono-Bold.woff2` | Space Mono 400 and 700 | SIL Open Font Licence 1.1, `SpaceMono-OFL.txt` |

They are Google Fonts' Latin subset (U+0000–00FF and common punctuation),
downloaded from fonts.gstatic.com in November 2026. Characters outside it
fall back to the system font. `style.css` loads them with `@font-face`.
