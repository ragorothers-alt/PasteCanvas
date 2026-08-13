// Generates simple PasteCanvas extension icons (clipboard glyph on accent bg)
// using only Node built-ins (zlib). Produces valid PNGs at 16/48/128.
const zlib = require("zlib");
const fs = require("fs");
const path = require("path");

const ACCENT = [108, 159, 255]; // #6c9fff
const BG = [26, 26, 46]; // #1a1a2e
const WHITE = [255, 255, 255];

function crc32(buf) {
  let table = crc32.table;
  if (!table) {
    table = crc32.table = new Int32Array(256);
    for (let n = 0; n < 256; n++) {
      let c = n;
      for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      table[n] = c;
    }
  }
  let crc = -1;
  for (let i = 0; i < buf.length; i++) crc = (crc >>> 8) ^ table[(crc ^ buf[i]) & 0xff];
  return (crc ^ -1) >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length, 0);
  const typeBuf = Buffer.from(type, "ascii");
  const crcBuf = Buffer.alloc(4);
  crcBuf.writeUInt32BE(crc32(Buffer.concat([typeBuf, data])), 0);
  return Buffer.concat([len, typeBuf, data, crcBuf]);
}

// Draws a clipboard glyph: returns [r,g,b] for a pixel.
function pixel(x, y, size) {
  const t = size / 128; // scale factor relative to 128 base
  // Rounded-corner background check
  const cx = Math.min(x, size - 1 - x);
  const cy = Math.min(y, size - 1 - y);
  const r = 18 * t;
  const inCorner = cx < r && cy < r;
  if (inCorner) {
    const dx = r - cx, dy = r - cy;
    if (dx * dx + dy * dy > r * r) return [0, 0, 0, 0]; // transparent corner
  }

  // Clipboard body (rounded rect)
  const bx0 = 34 * t, bx1 = 94 * t, by0 = 34 * t, by1 = 104 * t;
  const inBody = x >= bx0 && x <= bx1 && y >= by0 && y <= by1;

  // Clipboard clip (top tab)
  const cx0 = 50 * t, cx1 = 78 * t, cy0 = 24 * t, cy1 = 44 * t;
  const inClip = x >= cx0 && x <= cx1 && y >= cy0 && y <= cy1;

  // Text lines on the clipboard
  const lineH = 4 * t;
  const l1 = y >= 56 * t && y <= 56 * t + lineH && x >= 44 * t && x <= 84 * t;
  const l2 = y >= 68 * t && y <= 68 * t + lineH && x >= 44 * t && x <= 84 * t;
  const l3 = y >= 80 * t && y <= 80 * t + lineH && x >= 44 * t && x <= 72 * t;

  if (l1 || l2 || l3) return [...ACCENT, 255];
  if (inClip) return [...BG, 255];
  if (inBody) return [...WHITE, 255];
  return [...ACCENT, 255]; // background accent
}

function encodePNG(size) {
  const raw = Buffer.alloc(size * (size * 4 + 1));
  let off = 0;
  for (let y = 0; y < size; y++) {
    raw[off++] = 0; // filter type none
    for (let x = 0; x < size; x++) {
      const [r, g, b, a] = pixel(x, y, size);
      raw[off++] = r;
      raw[off++] = g;
      raw[off++] = b;
      raw[off++] = a;
    }
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // color type RGBA
  const idat = zlib.deflateSync(raw);
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr),
    chunk("IDAT", idat),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

const outDir = path.join(__dirname, "icons");
fs.mkdirSync(outDir, { recursive: true });
for (const size of [16, 48, 128]) {
  const png = encodePNG(size);
  fs.writeFileSync(path.join(outDir, `icon${size}.png`), png);
  console.log(`wrote icon${size}.png (${png.length} bytes)`);
}
