'use strict';
const path = require('path');
const { collectFiles, relativePath } = require('./templates/scripts/registry/file-utils.js');
const sialia = 'C:/Atiz/Competi/projetos/sialia/backend/Sialia.Backend';
const allFiles = collectFiles(sialia, '.cs');
const qrFolders = [];
for (const f of allFiles) {
  const rel = relativePath(sialia, f);
  const base = path.basename(f, '.cs');
  if (base.endsWith('QueryResolver')) {
    const dir = path.dirname(rel).replace(/\\/g, '/');
    qrFolders.push(dir);
  }
}
const unique = [...new Set(qrFolders)];
console.log('Unique folders with QueryResolver:');
unique.forEach(d => console.log('  ' + d));
// Find common segments
const first = unique[0].split('/');
const common = first.filter(seg => unique.every(f => f.split('/').includes(seg)));
console.log('\nCommon segments:', common);
console.log('Last common:', common[common.length - 1]);
