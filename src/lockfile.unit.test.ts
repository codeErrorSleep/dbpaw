import { describe, expect, test } from "bun:test";
import { readFileSync } from "fs";
import { resolve } from "path";

// Load and parse the package-lock.json relative to the project root
const lockfilePath = resolve(import.meta.dir, "../package-lock.json");
const lockfile = JSON.parse(readFileSync(lockfilePath, "utf-8"));

describe("package-lock.json version constraints (PR #changes)", () => {
  describe("root package version", () => {
    test("top-level version is 0.2.6", () => {
      expect(lockfile.version).toBe("0.2.6");
    });

    test("packages[''] version matches top-level version", () => {
      expect(lockfile.packages[""].version).toBe(lockfile.version);
    });
  });

  describe("@tauri-apps/plugin-process dependency constraint", () => {
    test("root package declares constraint ^2.3.1", () => {
      const deps = lockfile.packages[""].dependencies;
      expect(deps["@tauri-apps/plugin-process"]).toBe("^2.3.1");
    });

    test("resolved module version is 2.3.1", () => {
      const resolved =
        lockfile.packages["node_modules/@tauri-apps/plugin-process"];
      expect(resolved.version).toBe("2.3.1");
    });

    test("constraint uses caret range (not tilde)", () => {
      const constraint =
        lockfile.packages[""].dependencies["@tauri-apps/plugin-process"];
      expect(constraint.startsWith("^")).toBe(true);
      expect(constraint.startsWith("~")).toBe(false);
    });
  });

  describe("@tauri-apps/plugin-updater dependency constraint", () => {
    test("root package declares constraint ^2.10.0", () => {
      const deps = lockfile.packages[""].dependencies;
      expect(deps["@tauri-apps/plugin-updater"]).toBe("^2.10.0");
    });

    test("resolved module version is 2.10.0", () => {
      const resolved =
        lockfile.packages["node_modules/@tauri-apps/plugin-updater"];
      expect(resolved.version).toBe("2.10.0");
    });

    test("constraint uses caret range (not tilde)", () => {
      const constraint =
        lockfile.packages[""].dependencies["@tauri-apps/plugin-updater"];
      expect(constraint.startsWith("^")).toBe(true);
      expect(constraint.startsWith("~")).toBe(false);
    });
  });

  describe("cosmiconfig bundled yaml version", () => {
    test("cosmiconfig/node_modules/yaml is pinned to 1.10.2", () => {
      const yaml =
        lockfile.packages["node_modules/cosmiconfig/node_modules/yaml"];
      expect(yaml).toBeDefined();
      expect(yaml.version).toBe("1.10.2");
    });
  });

  describe("picomatch version", () => {
    test("picomatch resolved version is 4.0.3", () => {
      const picomatch = lockfile.packages["node_modules/picomatch"];
      expect(picomatch).toBeDefined();
      expect(picomatch.version).toBe("4.0.3");
    });

    test("picomatch entry does not have a resolved field", () => {
      // The PR diff removed the 'resolved' field from picomatch
      const picomatch = lockfile.packages["node_modules/picomatch"];
      expect(picomatch.resolved).toBeUndefined();
    });

    test("picomatch entry does not have an integrity field", () => {
      // The PR diff removed the 'integrity' field from picomatch
      const picomatch = lockfile.packages["node_modules/picomatch"];
      expect(picomatch.integrity).toBeUndefined();
    });
  });

  describe("lockfile structural integrity", () => {
    test("lockfile version is 3", () => {
      expect(lockfile.lockfileVersion).toBe(3);
    });

    test("requires is true", () => {
      expect(lockfile.requires).toBe(true);
    });

    test("name is dbpaw", () => {
      expect(lockfile.name).toBe("dbpaw");
    });

    test("both tauri plugin constraints are more specific than their previous tilde ranges", () => {
      // ~2 would allow 2.x.x; ^2.3.1 pins to >=2.3.1 <3.0.0 and is more specific
      const deps = lockfile.packages[""].dependencies;
      const processConstraint = deps["@tauri-apps/plugin-process"];
      const updaterConstraint = deps["@tauri-apps/plugin-updater"];

      // Verify they are not loose tilde-2 constraints
      expect(processConstraint).not.toBe("~2");
      expect(updaterConstraint).not.toBe("~2");

      // Verify they contain explicit patch version info
      expect(processConstraint).toMatch(/\^2\.\d+\.\d+/);
      expect(updaterConstraint).toMatch(/\^2\.\d+\.\d+/);
    });
  });
});