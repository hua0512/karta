# Renderer fonts

`HYSongYunLangHeiW-1.ttf` (宋韵朗黑) is the native card's existing face for
titles, body text and metadata. Titles use a 34px size and 46px line height.
The renderer loads an on-disk override from `resources/fonts/` when present,
and otherwise uses the embedded copy directly.

Fonts are loaded once; rendering a new link does not download fonts.
