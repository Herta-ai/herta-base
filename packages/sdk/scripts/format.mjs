import { readdir, readFile, stat, writeFile } from 'node:fs/promises';
import path from 'node:path';

import { format } from 'oxfmt';

const check = process.argv.includes('--check');
const roots = [
  'src',
  'tests',
  'docs',
  'README.md',
  'package.json',
  'tsconfig.json',
  'tsdown.config.ts',
];
const supported = new Set(['.ts', '.md', '.json']);
const files = [];

async function collect(entry) {
  const information = await stat(entry);
  if (information.isDirectory()) {
    for (const child of await readdir(entry)) await collect(path.join(entry, child));
  } else if (supported.has(path.extname(entry))) {
    files.push(entry);
  }
}

for (const root of roots) await collect(root);

let changed = 0;
for (const file of files) {
  const source = await readFile(file, 'utf8');
  const result = await format(file, source);
  if (result.errors.length) {
    throw new Error(`Unable to format ${file}: ${result.errors.map((error) => error.message).join('; ')}`);
  }
  if (result.code === source) continue;
  changed += 1;
  if (check) console.error(`Needs formatting: ${file}`);
  else await writeFile(file, result.code, 'utf8');
}

if (check && changed) process.exitCode = 1;
else console.log(check ? `Checked ${files.length} files` : `Formatted ${changed} files`);
