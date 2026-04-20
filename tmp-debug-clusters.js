'use strict';
const path = require('path');
const { collectFiles, relativePath } = require('./templates/scripts/registry/file-utils.js');
const MIN_SUFFIX_LENGTH = 6;
const MIN_FILES_PER_SUFFIX = 5;

function splitPascalCase(s) {
  return s.split(/(?=[A-Z][a-z])|(?<=[a-z])(?=[A-Z])/).filter(Boolean);
}

const sialia = 'C:/Atiz/Competi/projetos/sialia/backend/Sialia.Backend';
const allFiles = collectFiles(sialia, '.cs');

const suffixToFiles = new Map();
for (const f of allFiles) {
  const rel = relativePath(sialia, f);
  const dir = path.dirname(rel).replace(/\\/g, '/');
  const base = path.basename(f, '.cs');
  const words = splitPascalCase(base);
  if (words.length < 2) continue;
  for (let i = 1; i < words.length; i++) {
    const suffix = words.slice(words.length - i).join('');
    if (suffix.length < MIN_SUFFIX_LENGTH) continue;
    if (!suffixToFiles.has(suffix)) suffixToFiles.set(suffix, []);
    suffixToFiles.get(suffix).push({ base, folder: dir, file: path.basename(rel) });
  }
}

for (const [s, files] of suffixToFiles) {
  if (files.length < MIN_FILES_PER_SUFFIX) suffixToFiles.delete(s);
}

const qr = suffixToFiles.get('QueryResolver');
const r = suffixToFiles.get('Resolver');
console.log('QueryResolver count:', qr ? qr.length : 'NOT FOUND');
console.log('Resolver count:', r ? r.length : 'NOT FOUND');

if (qr && r) {
  const qrBases = new Set(qr.map(f => f.base));
  const rBases = new Set(r.map(f => f.base));
  const allQRinR = [...qrBases].every(b => rBases.has(b));
  console.log('All QR files in Resolver files?', allQRinR, '(expected: true, since *QueryResolver ends with Resolver)');
  console.log('Same size?', qrBases.size === rBases.size, '(25 vs 31 — should be false)');
}

// Show top 15 by fileCount
const top = [...suffixToFiles.entries()]
  .sort((a, b) => b[1].length - a[1].length)
  .slice(0, 15);
console.log('\nTop 15 suffixes (before pruning):');
for (const [s, files] of top) {
  console.log(`  ${files.length}\t${s}`);
}
