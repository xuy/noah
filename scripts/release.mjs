#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import { readdir, readFile, writeFile, rm } from "node:fs/promises";
import { tmpdir, homedir } from "node:os";
import path from "node:path";
import process from "node:process";
import { spawn } from "node:child_process";

const ROOT = process.cwd();
const TAURI_CONF_PATH = path.join(ROOT, "apps", "desktop", "src-tauri", "tauri.conf.json");
const IS_MAC = process.platform === "darwin";
const BUNDLE_DIR = IS_MAC
  ? path.join(ROOT, "target", "universal-apple-darwin", "release", "bundle")
  : path.join(ROOT, "target", "release", "bundle");
const RELEASE_REPO = process.env.NOAH_RELEASE_REPO ?? "xuy/noah";
const UPDATE_CHANNEL = process.env.NOAH_UPDATE_CHANNEL ?? "byok";
const UPDATE_BASE_URL =
  process.env.NOAH_UPDATE_BASE_URL ?? `https://onnoah.app/${UPDATE_CHANNEL}/download`;
const R2_BUCKET = process.env.NOAH_R2_BUCKET ?? "noah-downloads";
const LATEST_JSON_ASSET = `${UPDATE_CHANNEL}-latest.json`;
// Public BYOK releases must not overwrite the main website's stable
// /download assets. They only mirror into the channel-specific R2 prefix.
const MIRROR_STABLE_INSTALLERS = process.env.NOAH_MIRROR_STABLE_INSTALLERS === "1";

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
    const isWin = process.platform === "win32";
    // On Windows we spawn through the shell so .cmd shims (pnpm/npx) resolve,
    // but Node does NOT auto-quote array args for shell:true — so any arg with a
    // space (our "Noah for Tinkerers_*.msi" bundle paths) gets word-split by the
    // shell, and gh/wrangler receive garbage. Quote space-containing args so the
    // paths arrive intact. (No-op on macOS/Linux where shell is false.)
    const finalArgs = isWin ? args.map((a) => (/\s/.test(a) ? `"${a}"` : a)) : args;
    const child = spawn(command, finalArgs, {
      stdio: "inherit",
      shell: isWin,
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
    ["rpm", ".rpm"],
    ["appimage", ".AppImage"],
    ["appimage", ".AppImage.sig"],
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

function updaterDownloadUrl(tag, fileName) {
  const params = new URLSearchParams({ tag, file: fileName });
  return `${UPDATE_BASE_URL}?${params.toString()}`;
}

function contentTypeFor(fileName) {
  if (fileName.endsWith(".json")) return "application/json; charset=utf-8";
  if (fileName.endsWith(".dmg")) return "application/x-apple-diskimage";
  return "application/octet-stream";
}

async function uploadR2Object(localPath, key, contentType = contentTypeFor(path.basename(localPath))) {
  await runCommand("npx", [
    "wrangler", "r2", "object", "put", `${R2_BUCKET}/${key}`,
    `--file=${localPath}`, `--content-type=${contentType}`, "--remote",
  ]);
}

async function generateLatestJson(version, tag, artifacts) {
  const target = UPDATER_PLATFORM_MAP[`${process.platform}-${process.arch}`];
  if (!target) {
    console.log(`==> Skipping ${LATEST_JSON_ASSET} — unknown platform: ${process.platform}-${process.arch}`);
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
    console.log(`==> Skipping ${LATEST_JSON_ASSET} — missing updater artifact or signature`);
    console.log(`    updaterFile: ${updaterFile}, sigFile: ${sigFile}`);
    return null;
  }

  const signature = (await readFile(sigFile, "utf8")).trim();
  const url = updaterDownloadUrl(tag, updaterFile);

  // Download existing channel metadata from the GitHub release to merge
  // platforms from serialized matrix builds. Do not use the asset name
  // "latest.json" here: legacy installs may still poll that legacy
  // xuy/noah release URL.
  const latestPath = path.join(ROOT, LATEST_JSON_ASSET);
  let existing = { version, pub_date: new Date().toISOString(), platforms: {} };
  try {
    const tmpDir = path.join(tmpdir(), `noah-release-${Date.now()}`);
    await runCommand("gh", [
      "release", "download", tag, "--repo", RELEASE_REPO,
      "--pattern", LATEST_JSON_ASSET, "-D", tmpDir,
    ]);
    const tmpLatest = path.join(tmpDir, LATEST_JSON_ASSET);
    if (existsSync(tmpLatest)) {
      const prev = JSON.parse(await readFile(tmpLatest, "utf8"));
      if (prev.platforms) {
        existing.platforms = prev.platforms;
      }
      await rm(tmpDir, { recursive: true });
      console.log(`    Merged platforms from existing ${LATEST_JSON_ASSET} in release`);
    }
  } catch {
    console.log(`    No existing ${LATEST_JSON_ASSET} in release — starting fresh`);
  }

  existing.version = version;
  existing.pub_date = new Date().toISOString();

  // Universal macOS binary serves both architectures
  if (IS_MAC) {
    existing.platforms["darwin-aarch64"] = { url, signature };
    existing.platforms["darwin-x86_64"] = { url, signature };
    console.log(`==> Generated ${LATEST_JSON_ASSET} with platforms darwin-aarch64 + darwin-x86_64 (universal)`);
  } else {
    existing.platforms[target] = { url, signature };
    console.log(`==> Generated ${LATEST_JSON_ASSET} with platform ${target}`);
  }

  await writeFile(latestPath, JSON.stringify(existing, null, 2) + "\n");
  return latestPath;
}

function expandHome(p) {
  if (!p) return p;
  if (p === "~") return homedir();
  if (p.startsWith("~/")) return path.join(homedir(), p.slice(2));
  return p;
}

// Load BYOK release signing secrets from a local, git-ignored env file so they
// no longer have to live in the shell profile. Simple KEY=VALUE list at
// ~/.noah-signing/byok.env (override with NOAH_SIGNING_ENV). This is the
// BYOK-only signing key (CE75B852), SEPARATE from the paid app's legacy key in
// ~/.noah-signing/desktop.env. byok.env is AUTHORITATIVE: it overrides the
// ambient shell env (which may still export the legacy paid key and would
// otherwise mis-sign a BYOK build). This is local-only — CI has no byok.env
// (early return below), so CI secrets are never touched.
function loadSigningEnv() {
  const envPath = expandHome(process.env.NOAH_SIGNING_ENV || "~/.noah-signing/byok.env");
  if (!existsSync(envPath)) {
    console.log(`==> No signing env file at ${envPath} — relying on current environment`);
    return;
  }
  let loaded = 0;
  for (const line of readFileSync(envPath, "utf8").split("\n")) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const eq = trimmed.indexOf("=");
    if (eq === -1) continue;
    const key = trimmed.slice(0, eq).trim();
    let val = trimmed.slice(eq + 1).trim();
    if (
      (val.startsWith('"') && val.endsWith('"')) ||
      (val.startsWith("'") && val.endsWith("'"))
    ) {
      val = val.slice(1, -1);
    }
    process.env[key] = val; // authoritative: override the shell env
    loaded += 1;
  }
  // byok.env pins the key by file path. Inline it AND clobber any stale inline
  // key (e.g. the legacy key still exported in the shell) so Tauri signs with
  // the BYOK key, not whatever the shell had.
  if (process.env.TAURI_SIGNING_PRIVATE_KEY_FILE) {
    const keyPath = expandHome(process.env.TAURI_SIGNING_PRIVATE_KEY_FILE);
    if (existsSync(keyPath)) {
      process.env.TAURI_SIGNING_PRIVATE_KEY = readFileSync(keyPath, "utf8").trim();
    }
  }
  console.log(`==> Loaded ${loaded} signing var(s) from ${envPath} (authoritative over shell)`);
}

// Tauri notarizes + staples the .app but only *signs* the .dmg it then bundles.
// A downloaded dmg carries the quarantine bit and Gatekeeper assesses the dmg
// itself — so the dmg must also be notarized + stapled, or users hit
// "Apple could not verify…" on open. Tauri doesn't do this; we do it here.
async function notarizeAndStapleDmgs(artifacts) {
  if (process.platform !== "darwin") return;
  const { APPLE_ID, APPLE_PASSWORD, APPLE_TEAM_ID } = process.env;
  if (!APPLE_ID || !APPLE_PASSWORD || !APPLE_TEAM_ID) {
    console.warn("==> Skipping dmg notarization — APPLE_ID/APPLE_PASSWORD/APPLE_TEAM_ID not set");
    return;
  }
  const dmgs = artifacts.filter((a) => a.endsWith(".dmg"));
  for (const dmg of dmgs) {
    console.log(`==> Notarizing dmg (submit + wait): ${path.basename(dmg)}`);
    await runCommand("xcrun", [
      "notarytool", "submit", dmg,
      "--apple-id", APPLE_ID, "--password", APPLE_PASSWORD, "--team-id", APPLE_TEAM_ID,
      "--wait",
    ]);
    console.log(`==> Stapling dmg: ${path.basename(dmg)}`);
    await runCommand("xcrun", ["stapler", "staple", dmg]);
  }
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  loadSigningEnv();
  const version = await readVersion();
  const tag = args.tag || `v${version}`;
  const uploading = args.mode === "upload";

  console.log(`==> Building noah ${tag} on ${process.platform}/${process.arch}`);

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

  // macOS code signing + notarization
  // Requires env vars: APPLE_SIGNING_IDENTITY, APPLE_ID, APPLE_PASSWORD, APPLE_TEAM_ID
  if (process.platform === "darwin") {
    const hasSigning = !!process.env.APPLE_SIGNING_IDENTITY;
    const hasNotarization = process.env.APPLE_ID && process.env.APPLE_PASSWORD && process.env.APPLE_TEAM_ID;
    if (!hasSigning) {
      console.warn("==> APPLE_SIGNING_IDENTITY not set — build will not be signed");
    }
    if (!hasNotarization) {
      console.warn("==> APPLE_ID/APPLE_PASSWORD/APPLE_TEAM_ID not set — build will not be notarized");
    }
    if (hasSigning && hasNotarization) {
      console.log("==> macOS signing + notarization enabled");
    }
  }

  if (process.platform === "linux") {
    process.env.NO_STRIP = 'true';
  }

  // macOS universal binary: ensure x86_64 target is installed
  if (IS_MAC) {
    console.log("==> Ensuring x86_64-apple-darwin Rust target is installed...");
    await runCommand("rustup", ["target", "add", "x86_64-apple-darwin"]);
  }

  console.log("==> Running tauri build...");
  const tauriBuildArgs = ["--filter", "@noah/desktop", "tauri", "build"];
  if (IS_MAC) {
    tauriBuildArgs.push("--target", "universal-apple-darwin");
    console.log("==> Building universal binary (ARM + Intel)");
  }
  await runCommand("pnpm", tauriBuildArgs);

  const artifacts = await collectArtifacts();
  if (artifacts.length === 0) {
    throw new Error(`No build artifacts found in ${BUNDLE_DIR}`);
  }

  console.log("==> Artifacts:");
  for (const artifact of artifacts) {
    console.log(`    ${artifact}`);
  }

  // Notarize + staple the dmg(s) — Tauri only signs them (see notarizeAndStapleDmgs).
  await notarizeAndStapleDmgs(artifacts);

  if (!uploading) {
    console.log("==> Build-only mode complete.");
    return;
  }

  if (!(await hasCommand("gh"))) {
    throw new Error("Missing required command: gh");
  }

  // Generate channel-specific updater metadata for Tauri.
  const latestJsonPath = await generateLatestJson(version, tag, artifacts);

  console.log(`==> Uploading to GitHub release ${RELEASE_REPO} ${tag}...`);
  let releaseExists = true;
  try {
    await runCommand("gh", ["release", "view", tag, "--repo", RELEASE_REPO]);
  } catch {
    releaseExists = false;
  }

  if (!releaseExists) {
    const createArgs = [
      "release", "create", tag, "--repo", RELEASE_REPO,
      "--title", `Noah ${tag}`, "--generate-notes",
    ];
    // BYOK (any non-"desktop" channel) releases on xuy/noah must NEVER become
    // the repo's "Latest" release. Legacy 1.1.0 installs resolve updates via
    //   github.com/xuy/noah/releases/latest/download/latest.json
    // and that pointer must keep resolving to the one-hop migration release
    // (which carries latest.json). A BYOK release tagged higher would steal
    // "Latest", 404 the legacy channel, and strand un-migrated users. Marking
    // BYOK releases as prereleases keeps them off the "Latest" pointer.
    if (UPDATE_CHANNEL !== "desktop") createArgs.push("--prerelease");
    await runCommand("gh", createArgs);
  }

  const toUpload = [...artifacts];
  if (latestJsonPath) toUpload.push(latestJsonPath);

  // Create stable-named copies for GitHub release convenience. These do not
  // drive the BYOK updater; updater URLs point at versioned onnoah.app R2 keys.
  const STABLE_NAMES = {
    ".dmg": "Noah.dmg",
    "-setup.exe": "Noah-setup.exe",
    ".msi": "Noah.msi",
    ".AppImage": "Noah.AppImage",
  };
  const stableCopies = [];
  for (const artifact of artifacts) {
    const name = path.basename(artifact);
    for (const [suffix, stableName] of Object.entries(STABLE_NAMES)) {
      if (name.endsWith(suffix) && !name.endsWith(".sig")) {
        const stablePath = path.join(path.dirname(artifact), stableName);
        const { copyFileSync } = await import("node:fs");
        copyFileSync(artifact, stablePath);
        stableCopies.push(stablePath);
        console.log(`    Stable copy: ${name} → ${stableName}`);
      }
    }
  }
  toUpload.push(...stableCopies);

  await runCommand("gh", [
    "release", "upload", tag, ...toUpload,
    "--repo", RELEASE_REPO, "--clobber",
  ]);
  await runCommand("gh", [
    "release", "view", tag, "--repo", RELEASE_REPO, "--json", "url", "-q", ".url",
  ]);

  // Mirror versioned artifacts and channel metadata to Cloudflare R2 so
  // https://onnoah.app/byok/latest.json can serve public BYOK updates
  // without touching the legacy GitHub latest.json channel.
  for (const artifact of artifacts) {
    const key = `${UPDATE_CHANNEL}/${tag}/${path.basename(artifact)}`;
    try {
      await uploadR2Object(artifact, key);
      console.log(`    R2: ${key} → ${R2_BUCKET}`);
    } catch (e) {
      console.log(`    R2 upload skipped for ${key} (${e.message})`);
    }
  }

  if (latestJsonPath) {
    try {
      await uploadR2Object(
        latestJsonPath,
        `${UPDATE_CHANNEL}/latest.json`,
        "application/json; charset=utf-8",
      );
      console.log(`    R2: ${UPDATE_CHANNEL}/latest.json → ${R2_BUCKET}`);
    } catch (e) {
      console.log(`    R2 upload skipped for ${UPDATE_CHANNEL}/latest.json (${e.message})`);
    }
  }

  if (MIRROR_STABLE_INSTALLERS) {
    for (const stablePath of stableCopies) {
      const key = path.basename(stablePath);
      try {
        await uploadR2Object(stablePath, key);
        console.log(`    R2: ${key} → ${R2_BUCKET}`);
      } catch (e) {
        console.log(`    R2 upload skipped for ${key} (${e.message})`);
      }
    }
  }

  // Clean up local channel metadata.
  if (latestJsonPath && existsSync(latestJsonPath)) {
    await rm(latestJsonPath);
  }
}

main().catch((error) => {
  console.error(`ERROR: ${error.message}`);
  process.exit(1);
});
