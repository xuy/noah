#!/usr/bin/env node

import { existsSync } from "node:fs";
import { readdir, readFile, writeFile, rm } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { spawn } from "node:child_process";

const ROOT = process.cwd();
const TAURI_CONF_PATH = path.join(ROOT, "apps", "desktop", "src-tauri", "tauri.conf.json");
const BUNDLE_DIR = path.join(ROOT, "target", "release", "bundle");
const REPO = "xuy/noah";

function usage() {
  console.log(`Usage:
  node scripts/release.mjs --build [--tag vX.Y.Z] [--skip-install]
  node scripts/release.mjs --upload [--tag vX.Y.Z] [--skip-install]

Flags:
  --build         Build only (default when no mode is passed)
  --upload        Build and upload artifacts to GitHub release
  --tag           Override release tag (default: v{tauri.conf.json version})
  --skip-install  Skip 'pnpm install --frozen-lockfile'
  --help          Show this help
`);
}

function parseArgs(argv) {
  const result = {
    mode: "build",
    tag: "",
    skipInstall: false,
  };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--build") {
      result.mode = "build";
      continue;
    }
    if (arg === "--upload") {
      result.mode = "upload";
      continue;
    }
    if (arg === "--skip-install") {
      result.skipInstall = true;
      continue;
    }
    if (arg === "--tag") {
      const next = argv[i + 1];
      if (!next) {
        throw new Error("Missing value for --tag");
      }
      result.tag = next;
      i += 1;
      continue;
    }
    if (arg === "--help" || arg === "-h") {
      usage();
      process.exit(0);
    }
    throw new Error(`Unknown argument: ${arg}`);
  }

  return result;
}

function runCommand(command, args) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      stdio: "inherit",
      shell: process.platform === "win32",
      cwd: ROOT,
    });
    child.on("error", reject);
    child.on("close", (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`${command} ${args.join(" ")} failed with exit code ${code}`));
      }
    });
  });
}

function hasCommand(command) {
  const checker = process.platform === "win32" ? "where" : "which";
  return new Promise((resolve) => {
    const child = spawn(checker, [command], {
      stdio: "ignore",
      shell: process.platform === "win32",
    });
    child.on("close", (code) => resolve(code === 0));
    child.on("error", () => resolve(false));
  });
}

async function readVersion() {
  if (!existsSync(TAURI_CONF_PATH)) {
    throw new Error(`Missing ${TAURI_CONF_PATH}`);
  }
  const raw = await readFile(TAURI_CONF_PATH, "utf8");
  const json = JSON.parse(raw);
  if (!json.version) {
    throw new Error(`Could not read version from ${TAURI_CONF_PATH}`);
  }
  return String(json.version);
}

async function collectArtifacts() {
  const candidates = [
    ["dmg", ".dmg"],
    ["macos", ".tar.gz"],
    ["macos", ".tar.gz.sig"],
    ["msi", ".msi"],
    ["msi", ".msi.sig"],
    ["nsis", ".exe"],
    ["nsis", ".exe.sig"],
    ["deb", ".deb"],
    ["appimage", ".AppImage"],
  ];

  const artifacts = [];
  for (const [subdir, suffix] of candidates) {
    const dir = path.join(BUNDLE_DIR, subdir);
    if (!existsSync(dir)) continue;
    const files = await readdir(dir, { withFileTypes: true });
    for (const f of files) {
      if (!f.isFile()) continue;
      if (f.name.endsWith(suffix)) {
        artifacts.push(path.join(dir, f.name));
      }
    }
  }
  artifacts.sort();
  return artifacts;
}

// ── Updater JSON generation ─────────────────────────────────────────────
// Tauri v2 updater expects a JSON file at the endpoint with this shape:
// { "version": "X.Y.Z", "pub_date": "...", "platforms": { "<target>": { "url": "...", "signature": "..." } } }

const UPDATER_PLATFORM_MAP = {
  // [os, arch] → Tauri target key
  "darwin-arm64": "darwin-aarch64",
  "darwin-x64": "darwin-x86_64",
  "win32-x64": "windows-x86_64",
  "linux-x64": "linux-x86_64",
};

async function generateLatestJson(version, tag, artifacts) {
  const target = UPDATER_PLATFORM_MAP[`${process.platform}-${process.arch}`];
  if (!target) {
    console.log(`==> Skipping latest.json — unknown platform: ${process.platform}-${process.arch}`);
    return null;
  }

  // Find the updater artifact (.tar.gz on Mac, .nsis.zip or .exe on Windows, .AppImage on Linux)
  // and its corresponding .sig file.
  let updaterFile = null;
  let sigFile = null;

  for (const a of artifacts) {
    const name = path.basename(a);
    if (process.platform === "darwin" && name.endsWith(".tar.gz") && !name.endsWith(".sig")) {
      updaterFile = name;
    } else if (process.platform === "win32" && name.endsWith(".exe") && !name.endsWith(".sig")) {
      // Tauri v2 NSIS updater uses the .exe directly
      updaterFile = name;
    } else if (process.platform === "linux" && name.endsWith(".AppImage") && !name.endsWith(".sig")) {
      updaterFile = name;
    }
    // Collect sig
    if (name.endsWith(".tar.gz.sig") || name.endsWith(".exe.sig") || name.endsWith(".AppImage.sig")) {
      sigFile = a;
    }
  }

  if (!updaterFile || !sigFile) {
    console.log(`==> Skipping latest.json — missing updater artifact or signature`);
    console.log(`    updaterFile: ${updaterFile}, sigFile: ${sigFile}`);
    return null;
  }

  const signature = (await readFile(sigFile, "utf8")).trim();
  const url = `https://github.com/${REPO}/releases/download/${tag}/${updaterFile}`;

  // Try to load existing latest.json to merge platforms from multiple builds
  const latestPath = path.join(ROOT, "latest.json");
  let existing = { version, pub_date: new Date().toISOString(), platforms: {} };
  if (existsSync(latestPath)) {
    try {
      existing = JSON.parse(await readFile(latestPath, "utf8"));
    } catch { /* start fresh */ }
  }

  existing.version = version;
  existing.pub_date = new Date().toISOString();
  existing.platforms[target] = { url, signature };

  await writeFile(latestPath, JSON.stringify(existing, null, 2) + "\n");
  console.log(`==> Generated latest.json with platform ${target}`);
  return latestPath;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const version = await readVersion();
  const tag = args.tag || `v${version}`;
  const uploading = args.mode === "upload";

  console.log(`==> Building itman ${tag} on ${process.platform}/${process.arch}`);

  if (!(await hasCommand("pnpm"))) {
    throw new Error("Missing required command: pnpm");
  }

  if (!args.skipInstall) {
    console.log("==> Installing dependencies...");
    await runCommand("pnpm", ["install", "--frozen-lockfile"]);
  } else {
    console.log("==> Skipping dependency install (--skip-install)");
  }

  // Clean stale bundle artifacts from previous builds to avoid uploading wrong versions.
  if (existsSync(BUNDLE_DIR)) {
    console.log("==> Cleaning old bundle artifacts...");
    await rm(BUNDLE_DIR, { recursive: true, force: true });
  }

  // macOS code signing + notarization (set env vars or use defaults from keychain profile)
  if (process.platform === "darwin") {
    if (!process.env.APPLE_SIGNING_IDENTITY) {
      console.warn("==> APPLE_SIGNING_IDENTITY not set — macOS build will not be signed");
      console.warn("    Set it to your 'Developer ID Application: ...' identity");
    } else {
      process.env.APPLE_NOTARIZATION_CREDENTIALS = process.env.APPLE_NOTARIZATION_CREDENTIALS || "noah-notarize";
      console.log("==> macOS signing + notarization enabled");
    }
  }

  console.log("==> Running tauri build...");
  await runCommand("pnpm", ["--filter", "@itman/desktop", "tauri", "build"]);

  const artifacts = await collectArtifacts();
  if (artifacts.length === 0) {
    throw new Error(`No build artifacts found in ${BUNDLE_DIR}`);
  }

  console.log("==> Artifacts:");
  for (const artifact of artifacts) {
    console.log(`    ${artifact}`);
  }

  if (!uploading) {
    console.log("==> Build-only mode complete.");
    return;
  }

  if (!(await hasCommand("gh"))) {
    throw new Error("Missing required command: gh");
  }

  // Generate latest.json for the Tauri updater
  const latestJsonPath = await generateLatestJson(version, tag, artifacts);

  console.log(`==> Uploading to GitHub release ${tag}...`);
  let releaseExists = true;
  try {
    await runCommand("gh", ["release", "view", tag]);
  } catch {
    releaseExists = false;
  }

  if (!releaseExists) {
    await runCommand("gh", ["release", "create", tag, "--title", `Noah ${tag}`, "--generate-notes"]);
  }

  const toUpload = [...artifacts];
  if (latestJsonPath) toUpload.push(latestJsonPath);

  await runCommand("gh", ["release", "upload", tag, ...toUpload, "--clobber"]);
  await runCommand("gh", ["release", "view", tag, "--json", "url", "-q", ".url"]);

  // Clean up local latest.json
  if (latestJsonPath && existsSync(latestJsonPath)) {
    await rm(latestJsonPath);
  }
}

main().catch((error) => {
  console.error(`ERROR: ${error.message}`);
  process.exit(1);
});
