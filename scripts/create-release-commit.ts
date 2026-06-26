/* eslint-disable no-console */
/// <reference types="node" />

import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';

const version = readPackageVersion();

const tag = `v${version}`;
const message = `chore(release): ${tag}`;
const releaseFiles = [
  'package.json',
  'pnpm-lock.yaml',
  'CHANGELOG.md',
  'src-tauri/Cargo.toml',
  'src-tauri/Cargo.lock',
  'src-tauri/tauri.conf.json',
].filter((file) => existsSync(file));

git(['add', ...releaseFiles], { stdio: 'inherit' });

let hasStagedChanges = true;

try {
  git(['diff', '--cached', '--quiet']);
  hasStagedChanges = false;
} catch {
  hasStagedChanges = true;
}

if (hasStagedChanges) {
  git(['commit', '-m', message], { stdio: 'inherit' });
} else {
  console.log('No release changes to commit.');
}

try {
  git(['rev-parse', '--verify', '--quiet', tag]);
  console.log(`Tag ${tag} already exists.`);
} catch {
  git(['tag', '-a', tag, '-m', message], { stdio: 'inherit' });
}

function git(args: string[], options: { stdio?: 'pipe' | 'inherit' } = {}) {
  return execFileSync('git', args, {
    encoding: 'utf8',
    stdio: options.stdio ?? 'pipe',
  });
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function readPackageVersion() {
  const packageJson: unknown = JSON.parse(readFileSync('package.json', 'utf8'));

  if (!isRecord(packageJson) || typeof packageJson.version !== 'string' || !packageJson.version) {
    throw new Error('package.json version is missing');
  }

  return packageJson.version;
}
