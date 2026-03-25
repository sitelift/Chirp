import sharp from 'sharp';
import { writeFileSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));

async function pngToBmp24(inputPath, outputPath) {
  const { data, info } = await sharp(inputPath)
    .flatten({ background: { r: 26, g: 25, b: 23 } }) // #1a1917
    .raw()
    .toBuffer({ resolveWithObject: true });

  const { width, height, channels } = info;
  const rowSize = Math.ceil((width * 3) / 4) * 4; // rows padded to 4-byte boundary
  const pixelDataSize = rowSize * height;
  const fileSize = 54 + pixelDataSize; // 14 (file header) + 40 (info header) + pixels

  const buf = Buffer.alloc(fileSize);

  // BMP file header (14 bytes)
  buf.write('BM', 0);
  buf.writeUInt32LE(fileSize, 2);
  buf.writeUInt32LE(0, 6); // reserved
  buf.writeUInt32LE(54, 10); // pixel data offset

  // DIB header (BITMAPINFOHEADER, 40 bytes)
  buf.writeUInt32LE(40, 14); // header size
  buf.writeInt32LE(width, 18);
  buf.writeInt32LE(height, 22); // positive = bottom-up
  buf.writeUInt16LE(1, 26); // color planes
  buf.writeUInt16LE(24, 28); // bits per pixel
  buf.writeUInt32LE(0, 30); // no compression
  buf.writeUInt32LE(pixelDataSize, 34);
  buf.writeInt32LE(2835, 38); // h resolution (72 DPI)
  buf.writeInt32LE(2835, 42); // v resolution
  buf.writeUInt32LE(0, 46); // colors in palette
  buf.writeUInt32LE(0, 50); // important colors

  // Pixel data (bottom-up, BGR)
  for (let y = 0; y < height; y++) {
    const srcRow = (height - 1 - y) * width * channels; // flip vertically
    const dstRow = 54 + y * rowSize;
    for (let x = 0; x < width; x++) {
      const srcIdx = srcRow + x * channels;
      const dstIdx = dstRow + x * 3;
      buf[dstIdx] = data[srcIdx + 2];     // B
      buf[dstIdx + 1] = data[srcIdx + 1]; // G
      buf[dstIdx + 2] = data[srcIdx];     // R
    }
  }

  writeFileSync(outputPath, buf);
  console.log(`Created ${outputPath} (${width}x${height}, 24-bit BMP, ${fileSize} bytes)`);
}

await pngToBmp24(join(__dirname, 'chirp-sidebar.png'), join(__dirname, 'chirp-sidebar.bmp'));
await pngToBmp24(join(__dirname, 'chirp-header.png'), join(__dirname, 'chirp-header.bmp'));
console.log('Done!');
