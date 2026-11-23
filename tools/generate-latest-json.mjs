#!/usr/bin/env node
// Builds the Tauri updater manifest (`latest.json`) from the assets staged for
// a draft GitHub release.
//
// The manifest is served from
// https://github.com/<repo>/releases/latest/download/latest.json and is the
// only thing the app requests when it checks for updates. Each entry pairs an
// artifact URL with the minisign signature Tauri wrote next to that artifact.
// A missing asset or signature fails the release instead of publishing an
// update that every client would reject.
//
// Usage (from the repository root, after assets are copied into `release/`):
//   GITHUB_REPOSITORY=yjid/kensaku GITHUB_REF_NAME=v1.0.19 \
//     node tools/generate-latest-json.mjs

import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const releaseDir = process.env.RELEASE_DIR || "release";
const repository = process.env.GITHUB_REPOSITORY;
const tag = process.env.GITHUB_REF_NAME;

function fail(message) {
  console.error(`latest.json: ${message}`);
  process.exit(1);
}

if (!repository) fail("GITHUB_REPOSITORY is required");
if (!tag) fail("GITHUB_REF_NAME is required");

const config = JSON.parse(
  readFileSync("src-tauri/tauri.conf.json", "utf8"),
);
const version = config.version;
if (typeof version !== "string" || version.length === 0) {
  fail("src-tauri/tauri.conf.json has no version");
}
if (tag.replace(/^v/, "") !== version) {
  fail(`tag ${tag} does not match tauri.conf.json version ${version}`);
}

const assets = readdirSync(releaseDir);

function isArm(name) {
  return /(?:^|[_-])(?:aarch64|arm64)(?:[_-]|\.|$)/i.test(name);
}

function signatureFor(asset) {
  const path = join(releaseDir, `${asset}.sig`);
  let signature;
  try {
    signature = readFileSync(path, "utf8").trim();
  } catch {
    fail(`missing signature ${path}`);
  }
  if (signature.length === 0) fail(`empty signature ${path}`);

  // `requireSignedVersion` is enabled in the app, so the trusted comment must
  // name this exact version. Decode it here and fail the release early instead
  // of shipping an update every client rejects at install time.
  let signedText;
  try {
    signedText = Buffer.from(signature, "base64").toString("utf8");
  } catch {
    fail(`signature ${path} is not valid base64`);
  }
  if (!signedText.includes("trusted comment:")) {
    fail(`signature ${path} is not a minisign signature`);
  }
  const signedVersion = /(?:^|\s)version:([^\s]+)/.exec(signedText);
  if (!signedVersion) {
    fail(`signature ${path} is not bound to a version (sign with --app-version)`);
  }
  if (signedVersion[1] !== version) {
    fail(
      `signature ${path} is bound to ${signedVersion[1]}, expected ${version}`,
    );
  }
  return signature;
}

function urlFor(asset) {
  return `https://github.com/${repository}/releases/download/${encodeURIComponent(
    tag,
  )}/${encodeURIComponent(asset)}`;
}

const platforms = {};

function add(keys, asset) {
  const entry = { signature: signatureFor(asset), url: urlFor(asset) };
  for (const key of keys) {
    if (platforms[key]) fail(`both ${platforms[key].url} and ${asset} claim ${key}`);
    platforms[key] = entry;
  }
}

// Windows in-app updates install the per-user NSIS package. The extra
// `-nsis` keys keep working if binary patching is enabled later, and leave
// `windows-*-msi` absent on purpose: MSI installs update manually.
const setups = assets.filter((asset) => /-setup\.exe$/i.test(asset));
for (const asset of setups) {
  const base = isArm(asset) ? "windows-aarch64" : "windows-x86_64";
  add([base, `${base}-nsis`], asset);
}

// The macOS build is universal, so one archive serves both Intel and Apple
// Silicon. Tauri's updater looks for the `-app` target first and then falls
// back to the plain one.
const macArchives = assets.filter((asset) => /\.app\.tar\.gz$/i.test(asset));
if (macArchives.length > 1) {
  fail(`expected one universal macOS archive, found ${macArchives.length}`);
}
if (macArchives.length === 1) {
  add(
    [
      "darwin-aarch64",
      "darwin-aarch64-app",
      "darwin-x86_64",
      "darwin-x86_64-app",
    ],
    macArchives[0],
  );
}

// Linux in-app updates only work for AppImage. .deb and .rpm users stay on
// their package manager.
const appImages = assets.filter((asset) => /\.AppImage$/i.test(asset));
for (const asset of appImages) {
  const base = isArm(asset) ? "linux-aarch64" : "linux-x86_64";
  add([base, `${base}-appimage`], asset);
}

for (const required of ["windows-x86_64", "darwin-aarch64", "linux-x86_64"]) {
  if (!platforms[required]) fail(`no updater artifact for ${required}`);
}

const manifest = {
  version,
  pub_date: new Date().toISOString(),
  platforms,
};
const notes = process.env.RELEASE_NOTES;
if (notes && notes.trim().length > 0) manifest.notes = notes;

const output = join(releaseDir, "latest.json");
writeFileSync(output, `${JSON.stringify(manifest, null, 2)}\n`);

for (const [platform, entry] of Object.entries(platforms)) {
  console.log(`latest.json: ${platform} -> ${entry.url}`);
}
console.log(`latest.json: wrote ${output} for ${version}`);
