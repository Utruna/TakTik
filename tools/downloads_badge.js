/**
 * tools/downloads_badge.js
 * Run by .github/workflows/downloads-badge.yml:
 *
 *   node tools/downloads_badge.js <count> <out.svg>
 *
 * Writes the README's "Téléchargements" badge as a flat SVG. Drawn here
 * rather than by shields.io: the README then depends on GitHub alone (the
 * file is served by raw.githubusercontent.com), not on a third service
 * fetching a JSON file that can briefly 404 and be cached as an error badge.
 */

'use strict';

const fs = require('fs');

const LABEL = 'Téléchargements';
const LABEL_COLOR = '#555';
const MESSAGE_COLOR = '#4a90e2';
const HEIGHT = 20;
const CHAR_WIDTH = 6.6; // Verdana 11px, on average
const PADDING = 10;

const escapeXml = (text) => text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

function badge(label, message) {
  const labelWidth = Math.round(label.length * CHAR_WIDTH + PADDING * 2);
  const messageWidth = Math.round(message.length * CHAR_WIDTH + PADDING * 2);
  const width = labelWidth + messageWidth;
  const labelX = labelWidth / 2;
  const messageX = labelWidth + messageWidth / 2;
  const text = (x, content) => `
    <text x="${x}" y="15" fill="#010101" fill-opacity=".3">${escapeXml(content)}</text>
    <text x="${x}" y="14">${escapeXml(content)}</text>`;
  return `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${HEIGHT}" role="img" aria-label="${escapeXml(label)}: ${escapeXml(message)}">
  <title>${escapeXml(label)}: ${escapeXml(message)}</title>
  <linearGradient id="s" x2="0" y2="100%">
    <stop offset="0" stop-color="#bbb" stop-opacity=".1"/>
    <stop offset="1" stop-opacity=".1"/>
  </linearGradient>
  <clipPath id="r"><rect width="${width}" height="${HEIGHT}" rx="3" fill="#fff"/></clipPath>
  <g clip-path="url(#r)">
    <rect width="${labelWidth}" height="${HEIGHT}" fill="${LABEL_COLOR}"/>
    <rect x="${labelWidth}" width="${messageWidth}" height="${HEIGHT}" fill="${MESSAGE_COLOR}"/>
    <rect width="${width}" height="${HEIGHT}" fill="url(#s)"/>
  </g>
  <g fill="#fff" text-anchor="middle" font-family="Verdana,Geneva,DejaVu Sans,sans-serif" font-size="11" text-rendering="geometricPrecision">${text(labelX, label)}${text(messageX, message)}
  </g>
</svg>
`;
}

const [count, out] = process.argv.slice(2);
if (!/^\d+$/.test(count || '') || !out) {
  throw new Error('Usage: node tools/downloads_badge.js <count> <out.svg>');
}
fs.writeFileSync(out, badge(LABEL, count));
console.log(`[badge] ${LABEL}: ${count} -> ${out}`);
