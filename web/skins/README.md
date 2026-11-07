# Tile skins

Every tile on screen is drawn from the chosen skin. Players pick a skin with the 🎨 button;
the choice is kept in their browser and only changes what they see.

To add a skin, make a folder here with a `skin.json`, and add the folder name to `index.json`.
A skin can use any of these fields (all optional except `name`):

```json
{
  "name": "My skin",
  "description": "One line shown in the picker.",
  "vars": { "--tile": "#fff", "--man": "#c00" },
  "faces": { "1z": { "big": "E", "suit": "east" } },
  "images": "tiles/{code}.svg",
  "back": "tiles/back.svg"
}
```

- `vars` sets CSS variables: `--tile` and `--tile-edge` (face and edge), `--man`, `--pin`,
  `--sou` (suit colours), `--tile-honor`, `--tile-dragon-white`, `--tile-back`,
  `--tile-back-edge` and `--tile-font`.
- `faces` changes the text on a tile: `big` is the main symbol and `suit` the small line under it.
- `images` draws every tile from a picture instead. `{code}` becomes the tile's code:
  `1m`-`9m` (characters), `1p`-`9p` (dots), `1s`-`9s` (bamboo), `1z`-`4z` (East, South, West,
  North) and `5z`-`7z` (white, green, red dragon). Pictures should be about 60 × 84.
- `back` is the picture for face-down tiles.

`scripts/skins/make_pips.py` draws the Pips skin and is a starting point for a picture skin.
`npm test` in `scripts/ui-smoke` checks every skin has all its files.
