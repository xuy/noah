#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { spawn } from "node:child_process";

const ROOT = process.cwd();

const FILES = {
  tauriConf: path.join(ROOT, "apps", "desktop", "src-tauri", "tauri.conf.json"),
  packageJson: path.join(ROOT, "apps", "desktop", "package.json"),
  desktopCargo: path.join(ROOT, "apps", "desktop", "src-tauri", "Cargo.toml"),
};

function usage() {
  console.log(`Usage: node scripts/bump-version.mjs <version>

Example:
  node scripts/bump-version.mjs 0.13.0

Updates version in all project files, creates a git commit, and tags it.
`);
}

function runCommand(command, args) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { stdio: "inherit", cwd: ROOT });
    child.on("error", reject);
    child.on("close", (code) => {
      if (code === 0) resolve();
      else reject(new Error(`${command} ${args.join(" ")} exited with ${code}`));
    });
  });
}

async function updateJson(filePath, version) {
  const raw = await readFile(filePath, "utf8");
  const json = JSON.parse(raw);
  json.version = version;
  await writeFile(filePath, JSON.stringify(json, null, 2) + "\n");
  console.log(`  ✓ ${path.relative(ROOT, filePath)}`);
}

async function updateCargoToml(filePath, version) {
  const raw = await readFile(filePath, "utf8");
  // Replace the version in the [package] section (first occurrence)
  const updated = raw.replace(
    /^(version\s*=\s*)"[^"]*"/m,
    `$1"${version}"`
  );
  await writeFile(filePath, updated);
  console.log(`  ✓ ${path.relative(ROOT, filePath)}`);
}

async function main() {
  const version = process.argv[2];
  if (!version || version === "--help" || version === "-h") {
    usage();
    process.exit(version ? 0 : 1);
  }

  if (!/^\d+\.\d+\.\d+$/.test(version)) {
    console.error(`ERROR: Invalid version "${version}" — expected format X.Y.Z`);
    process.exit(1);
  }

  const tag = `v${version}`;
  console.log(`==> Bumping version to ${version} (tag: ${tag})\n`);

  console.log("Updating files:");
  await updateJson(FILES.tauriConf, version);
  await updateJson(FILES.packageJson, version);
  await updateCargoToml(FILES.desktopCargo, version);

  console.log("\nCreating git commit and tag...");
  await runCommand("git", ["add",
    FILES.tauriConf,
    FILES.packageJson,
    FILES.desktopCargo,
  ]);
  await runCommand("git", ["commit", "-m", `chore: bump version to ${version}`]);
  await runCommand("git", ["tag", "-a", tag, "-m", `Release ${tag}`]);

  console.log(`\n==> Done! Version bumped to ${version}`);
  console.log(`    Commit and tag ${tag} created.`);
  console.log(`    Run 'git push && git push --tags' when ready.`);
}

main().catch((err) => {
  console.error(`ERROR: ${err.message}`);
  process.exit(1);
});
