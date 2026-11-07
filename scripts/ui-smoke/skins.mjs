// Checks every skin listed in web/skins/index.json: its skin.json parses, only uses known
// fields, and (for picture skins) has an image for all 34 tiles plus the back.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const dir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../web/skins');
const codes = [];
for (const s of 'mps') for (let n = 1; n <= 9; n++) codes.push(`${n}${s}`);
for (let n = 1; n <= 7; n++) codes.push(`${n}z`);
const known = new Set(['name', 'description', 'vars', 'faces', 'images', 'back']);

const ids = JSON.parse(fs.readFileSync(path.join(dir, 'index.json'), 'utf8'));
for (const id of ids) {
  const skin = JSON.parse(fs.readFileSync(path.join(dir, id, 'skin.json'), 'utf8'));
  const fail = (msg) => { throw new Error(`skin ${id}: ${msg}`); };
  if (!skin.name) fail('needs a name');
  for (const k of Object.keys(skin)) if (!known.has(k)) fail(`unknown field "${k}"`);
  for (const k of Object.keys(skin.vars || {})) if (!k.startsWith('--')) fail(`"${k}" is not a CSS variable`);
  for (const k of Object.keys(skin.faces || {})) if (!codes.includes(k)) fail(`"${k}" is not a tile code`);
  if (skin.images) {
    for (const c of codes) {
      const file = path.join(dir, id, skin.images.replace('{code}', c));
      if (!fs.existsSync(file)) fail(`missing ${path.relative(dir, file)}`);
    }
  }
  if (skin.back && !fs.existsSync(path.join(dir, id, skin.back))) fail(`missing ${skin.back}`);
}
console.log(`skins ok: ${ids.join(', ')}`);
