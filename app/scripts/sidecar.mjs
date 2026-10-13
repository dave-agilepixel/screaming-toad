// Build the `toad` CLI and place it where Tauri's bundler expects the
// sidecar (bundle.externalBin "binaries/toad"): src-tauri/binaries/
// toad-<target-triple>[.exe]. Tauri then ships it next to the app
// executable as toad[.exe], which is where File > Scheduling runs it from.
//
//   node scripts/sidecar.mjs           release build (installers)
//   node scripts/sidecar.mjs --debug   debug build (tauri dev)

import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const debug = process.argv.includes("--debug");
const app = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const root = resolve(app, "..");

const host = execFileSync("rustc", ["-vV"], { encoding: "utf8" }).match(/^host: (.+)$/m)?.[1]?.trim();
if (!host) throw new Error("could not read the Rust host target from `rustc -vV`");
const triple = process.env.TAURI_ENV_TARGET_TRIPLE || host;
const ext = triple.includes("windows") ? ".exe" : "";

const args = ["build", "-p", "toad-cli"];
if (!debug) args.push("--release");
if (triple !== host) args.push("--target", triple);
console.log(`sidecar: cargo ${args.join(" ")}`);
execFileSync("cargo", args, { cwd: root, stdio: "inherit" });

const targetDir = process.env.CARGO_TARGET_DIR ? resolve(process.env.CARGO_TARGET_DIR) : join(root, "target");
const built = join(targetDir, ...(triple !== host ? [triple] : []), debug ? "debug" : "release", `toad${ext}`);
if (!existsSync(built)) throw new Error(`sidecar: ${built} was not built`);
const outDir = join(app, "src-tauri", "binaries");
mkdirSync(outDir, { recursive: true });
const out = join(outDir, `toad-${triple}${ext}`);
copyFileSync(built, out);
console.log(`sidecar: ${built} -> ${out}`);
