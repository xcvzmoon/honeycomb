/// <reference types="node" />

import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';

const packageJsonPath = 'package.json';
const tauriConfigPath = 'src-tauri/tauri.conf.json';
const cargoTomlPath = 'src-tauri/Cargo.toml';

const version = readPackageVersion();
const tauriConfig: unknown = JSON.parse(readFileSync(tauriConfigPath, 'utf8'));

if (!isRecord(tauriConfig)) {
  throw new Error('src-tauri/tauri.conf.json must contain an object');
}

tauriConfig.version = version;
writeFileSync(tauriConfigPath, `${JSON.stringify(tauriConfig, null, 2)}\n`);

const cargoToml = readFileSync(cargoTomlPath, 'utf8');
const versionPattern = /^(version = ")[^"]+("\s*)$/m;

if (!versionPattern.test(cargoToml)) {
  throw new Error('Could not find src-tauri/Cargo.toml package version');
}

const nextCargoToml = cargoToml.replace(versionPattern, `$1${version}$2`);
writeFileSync(cargoTomlPath, nextCargoToml);

execFileSync('cargo', ['metadata'], {
  cwd: 'src-tauri',
  stdio: 'inherit',
});

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function readPackageVersion() {
  const packageJson: unknown = JSON.parse(readFileSync(packageJsonPath, 'utf8'));

  if (!isRecord(packageJson) || typeof packageJson.version !== 'string' || !packageJson.version) {
    throw new Error('package.json version is missing');
  }

  return packageJson.version;
}
