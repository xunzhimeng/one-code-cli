#!/usr/bin/env node

const fs = require("node:fs");
const https = require("node:https");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const rootDir = path.resolve(__dirname, "..");
const packageJson = require(path.join(rootDir, "package.json"));
const binDir = path.join(__dirname, "bin");
const nativeName = process.platform === "win32" ? "occ-native.exe" : "occ-native";
const nativePath = path.join(binDir, nativeName);

const targets = {
  "win32-x64": { target: "x86_64-pc-windows-msvc", extension: ".exe" },
  "linux-x64": { target: "x86_64-unknown-linux-gnu", extension: "" },
  "darwin-x64": { target: "x86_64-apple-darwin", extension: "" },
  "darwin-arm64": { target: "aarch64-apple-darwin", extension: "" },
};

function fail(message) {
  console.error(message);
  process.exit(1);
}

function platformTarget() {
  const key = `${process.platform}-${process.arch}`;
  const item = targets[key];
  if (!item) {
    fail(`Unsupported platform for one-code-cli npm install: ${key}`);
  }
  return item;
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    stdio: "inherit",
    shell: process.platform === "win32",
    ...options,
  });
  return result.status === 0;
}

function copyLocalBuild() {
  const cargoToml = path.join(rootDir, "Cargo.toml");
  if (!fs.existsSync(cargoToml)) {
    return false;
  }

  console.log("Building occ from local Rust source...");
  if (!run("cargo", ["build", "--release"], { cwd: rootDir })) {
    return false;
  }

  const builtBinary = path.join(
    rootDir,
    "target",
    "release",
    process.platform === "win32" ? "occ.exe" : "occ"
  );

  if (!fs.existsSync(builtBinary)) {
    return false;
  }

  fs.mkdirSync(binDir, { recursive: true });
  fs.copyFileSync(builtBinary, nativePath);
  if (process.platform !== "win32") {
    fs.chmodSync(nativePath, 0o755);
  }
  return true;
}

function download(url, destination, redirects = 0) {
  return new Promise((resolve, reject) => {
    if (redirects > 5) {
      reject(new Error("Too many redirects"));
      return;
    }

    const request = https.get(
      url,
      {
        headers: {
          "User-Agent": `one-code-cli-npm/${packageJson.version}`,
        },
      },
      (response) => {
        if (
          response.statusCode >= 300 &&
          response.statusCode < 400 &&
          response.headers.location
        ) {
          response.resume();
          download(response.headers.location, destination, redirects + 1)
            .then(resolve)
            .catch(reject);
          return;
        }

        if (response.statusCode !== 200) {
          response.resume();
          reject(new Error(`HTTP ${response.statusCode} while downloading ${url}`));
          return;
        }

        fs.mkdirSync(path.dirname(destination), { recursive: true });
        const file = fs.createWriteStream(destination, { mode: 0o755 });
        response.pipe(file);
        file.on("finish", () => {
          file.close(() => resolve());
        });
        file.on("error", reject);
      }
    );

    request.on("error", reject);
  });
}

async function install() {
  if (process.env.OCC_SKIP_DOWNLOAD === "1") {
    console.log("Skipping occ native binary download because OCC_SKIP_DOWNLOAD=1.");
    return;
  }

  if (process.env.OCC_USE_LOCAL_BUILD === "1") {
    if (!copyLocalBuild()) {
      fail("Failed to build occ from local Rust source.");
    }
    return;
  }

  const selected = platformTarget();
  const ownerRepo = process.env.OCC_GITHUB_REPOSITORY || "xunzhimeng/one-code-cli";
  const tag = process.env.OCC_INSTALL_TAG || `v${packageJson.version}`;
  const assetName = `occ-${selected.target}${selected.extension}`;
  const url =
    process.env.OCC_DOWNLOAD_URL ||
    `https://github.com/${ownerRepo}/releases/download/${tag}/${assetName}`;

  console.log(`Downloading ${assetName} from ${url}`);

  try {
    await download(url, nativePath);
    if (process.platform !== "win32") {
      fs.chmodSync(nativePath, 0o755);
    }
    console.log(`Installed occ native binary to ${nativePath}`);
  } catch (error) {
    console.warn(error.message);
    console.warn("Falling back to local Rust build if source is available...");
    if (!copyLocalBuild()) {
      fail(
        "Failed to install occ native binary. Install from a published GitHub release, " +
          "set OCC_USE_LOCAL_BUILD=1, or install with Cargo."
      );
    }
  }
}

install().catch((error) => fail(error.message));
