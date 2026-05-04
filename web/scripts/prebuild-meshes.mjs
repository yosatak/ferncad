#!/usr/bin/env node
// Prebuild .fern → mesh JSON for the landing page.
//
// We invoke `cargo run -p ferncad-cli -- <input> --mesh-json <output>` for
// each source so the LP can ship pre-evaluated geometry without loading the
// WASM bundle. Runs from `web/` (cwd is wherever npm starts the script).
//
// In CI / local-docker environments, `cargo` must be on PATH (the dev image
// has it; CI installs it via dtolnay/rust-toolchain before npm run build).

import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, statSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(__dirname, '..', '..');
const lpDir = resolve(__dirname, '..', 'src', 'lp');

const TARGETS = [
  { src: 'hero.fern',             out: 'hero-mesh.json',                 segments: 16 },
  { src: 'sample-box.fern',       out: 'samples/box-mesh.json',          segments: 8 },
  { src: 'sample-sphere.fern',    out: 'samples/sphere-mesh.json',       segments: 24 },
  { src: 'sample-spur-gear.fern', out: 'samples/spur-gear-mesh.json',    segments: 20 },
  { src: 'sample-boolean.fern',   out: 'samples/boolean-mesh.json',      segments: 24 },
];

const sourcesDir = resolve(lpDir, 'sources');

function isUpToDate(srcPath, outPath) {
  if (!existsSync(outPath)) return false;
  return statSync(outPath).mtimeMs >= statSync(srcPath).mtimeMs;
}

let built = 0;
let skipped = 0;
const force = process.argv.includes('--force');

for (const target of TARGETS) {
  const src = resolve(sourcesDir, target.src);
  const out = resolve(lpDir, target.out);
  mkdirSync(dirname(out), { recursive: true });

  if (!force && isUpToDate(src, out)) {
    skipped++;
    continue;
  }

  const args = [
    'run', '--quiet', '--release', '-p', 'ferncad-cli', '--',
    src, '--mesh-json', out, '--segments', String(target.segments),
  ];
  const result = spawnSync('cargo', args, { cwd: repoRoot, stdio: 'inherit' });
  if (result.status !== 0) {
    console.error(`prebuild-meshes: cargo failed for ${target.src} (exit ${result.status})`);
    process.exit(result.status ?? 1);
  }
  built++;
}

console.log(`prebuild-meshes: ${built} rebuilt, ${skipped} up to date`);
