import { describe, expect, test } from "bun:test";
import { readFileSync } from "fs";
import { resolve } from "path";

// bun.lock is the authoritative lockfile for this project (bun install).
// package-lock.json is kept for npm compatibility but is NOT auto-updated by bun.
//
// bun.lock uses JSONC (allows trailing commas) — strip them before parsing.
function parseBunLock(path: string) {
  const raw = readFileSync(path, "utf-8");
  const json = raw.replace(/,(\s*[}\]])/g, "$1");
  return JSON.parse(json);
}

const lockfilePath = resolve(import.meta.dir, "../bun.lock");
const packageJsonPath = resolve(import.meta.dir, "../package.json");
const lockfile = parseBunLock(lockfilePath);
const packageJson = JSON.parse(readFileSync(packageJsonPath, "utf-8"));

describe("bun.lock integrity", () => {
  test("uses a modern bun lockfile structure", () => {
    expect(lockfile.lockfileVersion).toBe(1);
    expect(lockfile.workspaces).toBeDefined();
    expect(lockfile.workspaces[""]).toBeDefined();
  });

  test("workspace name matches package.json", () => {
    expect(lockfile.workspaces[""].name).toBe(packageJson.name);
  });

  test("runtime dependencies match package.json", () => {
    expect(lockfile.workspaces[""].dependencies).toEqual(
      packageJson.dependencies,
    );
  });
});
