// Builds the npm tarball from the CI artifact. It packs; it never publishes (docs/NPM-PACKAGE.md).
//
// usage: node wasm/npm/build.mjs <safeimage_wasm.wasm> <out dir> <commit> <workflow run url> <rustc version>
//
// The .wasm is copied byte for byte: nothing here rebuilds or rewrites it.
import { readFileSync, writeFileSync, mkdirSync, copyFileSync, rmSync } from "node:fs";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { join, dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import assert from "node:assert/strict";

const [wasmPath, outArg, commit, runUrl, rustc] = process.argv.slice(2);
assert.ok(wasmPath && outArg && commit && runUrl && rustc, "five arguments are required");
const outDir = resolve(outArg); // npm runs from the staging directory, so the destination must be absolute

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..", "..");

function cargoVersion(path) {
  const text = readFileSync(path, "utf8");
  const pkg = text.split(/^\[/m).find((s) => s.startsWith("package]"));
  const m = pkg && /^version\s*=\s*"([^"]+)"/m.exec(pkg);
  assert.ok(m, `no [package] version in ${path}`);
  return m[1];
}

// One number per project: the package carries the workspace version, and both crates must agree.
const version = cargoVersion(join(root, "Cargo.toml"));
assert.equal(cargoVersion(join(root, "wasm", "Cargo.toml")), version, "crate versions differ");

const wasm = readFileSync(wasmPath);
const sha256 = createHash("sha256").update(wasm).digest("hex");

// The ABI version comes from the module's own export, and the loader's constant must agree with it.
const mod = new WebAssembly.Module(wasm);
assert.deepEqual(WebAssembly.Module.imports(mod), [], "module must have no imports");
const abi = new WebAssembly.Instance(mod, {}).exports.si_abi_version();
const loader = readFileSync(join(here, "loader.js"), "utf8");
const declared = /export const ABI_VERSION = (\d+);/.exec(loader);
assert.ok(declared, "loader.js declares no ABI_VERSION");
assert.equal(Number(declared[1]), abi, "loader.js ABI_VERSION differs from the module's");

rmSync(outDir, { recursive: true, force: true });
const stage = join(outDir, "package");
mkdirSync(stage, { recursive: true });

const pkg = JSON.parse(readFileSync(join(here, "package.template.json"), "utf8"));
pkg.version = version;
writeFileSync(join(stage, "package.json"), JSON.stringify(pkg, null, 2) + "\n");
writeFileSync(
  join(stage, "PROVENANCE.json"),
  JSON.stringify({ commit, workflow_run_url: runUrl, wasm_sha256: sha256, abi_version: abi, rustc }, null, 2) + "\n",
);
copyFileSync(wasmPath, join(stage, "safeimage_wasm.wasm"));
for (const f of ["index.js", "loader.js", "safeimage.js", "codec.js", "index.d.ts", "README.md"]) copyFileSync(join(here, f), join(stage, f));
for (const f of ["LICENSE-MIT", "LICENSE-APACHE", "NOTICE"]) copyFileSync(join(root, f), join(stage, f));

// npm 10 prints an array of results, npm 12 an object keyed by package name.
const result = JSON.parse(
  execFileSync("npm", ["pack", "--json", "--pack-destination", outDir], { cwd: stage, encoding: "utf8", shell: process.platform === "win32" }),
);
const packed = Array.isArray(result) ? result[0] : Object.values(result)[0];

// The tarball holds exactly the listed files, and its .wasm is the artifact's.
const expected = [
  "LICENSE-APACHE", "LICENSE-MIT", "NOTICE", "PROVENANCE.json", "README.md",
  "codec.js", "index.d.ts", "index.js", "loader.js", "package.json", "safeimage.js", "safeimage_wasm.wasm",
];
assert.deepEqual(packed.files.map((f) => f.path).sort(), expected, "unexpected tarball contents");
// Run tar from the output directory with a relative name: GNU tar reads "C:" in an absolute Windows path as a host.
const inTar = execFileSync("tar", ["-xzOf", packed.filename, "package/safeimage_wasm.wasm"], { cwd: outDir, maxBuffer: 1 << 28 });
assert.equal(createHash("sha256").update(inTar).digest("hex"), sha256, "tarball .wasm differs from the artifact");

// The packed files work as a consumer would load them: import the entry point, instantiate the packed module.
const smoke = `import { SafeImage, wasmUrl } from ${JSON.stringify(pathToFileURL(join(stage, "index.js")).href)};
import { readFileSync } from "node:fs";
const si = await SafeImage.create(readFileSync(wasmUrl));
if (si.abiVersion !== ${abi}) throw new Error("abi");
if (si.classifyStatus(200) !== null || si.classifyStatus(429) !== "Throttled") throw new Error("classify");`;
execFileSync(process.execPath, ["--input-type=module", "-e", smoke], { stdio: "inherit" });

console.log(`built ${packed.filename} (version ${version}, abi ${abi}, wasm sha256 ${sha256})`);
