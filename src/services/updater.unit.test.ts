import { describe, expect, test, mock } from "bun:test";

let checkImpl: (...args: any[]) => Promise<any> = async () => ({
  available: false,
});

mock.module("@tauri-apps/plugin-updater", () => ({
  check: (...args: any[]) => checkImpl(...args),
}));

mock.module("@tauri-apps/plugin-process", () => ({
  relaunch: async () => {},
}));

const loadUpdater = async () =>
  import(`./updater?test=${Date.now()}-${Math.random()}`);

describe("updater checkForUpdates (mock mode)", () => {
  test("available scenario emits checking -> available", async () => {
    const updater = await loadUpdater();
    updater.enableMock("available");

    const states: string[] = [];
    const result = await updater.checkForUpdates({
      onStateChange: (s: string) => states.push(s),
    });

    expect(states[0]).toBe("checking");
    expect(states[states.length - 1]).toBe("available");
    expect(result.available).toBe(true);
    expect(result.state).toBe("available");
  });

  test("no_update scenario emits checking -> idle", async () => {
    const updater = await loadUpdater();
    updater.enableMock("no_update");

    const states: string[] = [];
    const result = await updater.checkForUpdates({
      onStateChange: (s: string) => states.push(s),
    });

    expect(states[0]).toBe("checking");
    expect(states[states.length - 1]).toBe("idle");
    expect(result.available).toBe(false);
    expect(result.errorCode).toBe("NO_UPDATE");
  });

  test("error scenario emits checking -> error", async () => {
    const updater = await loadUpdater();
    updater.enableMock("error");

    const states: string[] = [];
    const result = await updater.checkForUpdates({
      onStateChange: (s: string) => states.push(s),
    });

    expect(states[0]).toBe("checking");
    expect(states[states.length - 1]).toBe("error");
    expect(result.available).toBe(false);
    expect(result.errorCode).toBe("CHECK_FAILED");
  });
});

describe("updater startBackgroundInstall", () => {
  test("prevents concurrent install", async () => {
    const updater = await loadUpdater();

    let resolveInstall: (() => void) | null = null;
    checkImpl = async () => ({
      available: true,
      version: "1.0.0",
      date: new Date().toISOString(),
      body: "test",
      downloadAndInstall: async () =>
        new Promise<void>((resolve) => {
          resolveInstall = resolve;
        }),
    });

    const first = updater.startBackgroundInstall();
    const second = updater.startBackgroundInstall();

    expect(first.started).toBe(true);
    expect(second.started).toBe(false);

    const waitForResolve = async () => {
      const start = Date.now();
      while (!resolveInstall) {
        if (Date.now() - start > 2000) {
          throw new Error("downloadAndInstall did not start in time");
        }
        await new Promise((r) => setTimeout(r, 0));
      }
    };

    await waitForResolve();
    const finishInstall = resolveInstall as (() => void) | null;
    if (!finishInstall) {
      throw new Error("downloadAndInstall resolver was not captured");
    }
    finishInstall();
    const completion = await updater.waitForInstallCompletion();
    expect(completion?.state).toBe("ready_to_restart");
  });

  test("emits state transitions and completes", async () => {
    const updater = await loadUpdater();

    checkImpl = async () => ({
      available: true,
      version: "1.0.1",
      date: new Date().toISOString(),
      body: "test",
      downloadAndInstall: async () => {},
    });

    const states: string[] = [];
    const start = updater.startBackgroundInstall(undefined, {
      onStateChange: (s: string) => states.push(s),
    });

    expect(start.started).toBe(true);
    const completion = await updater.waitForInstallCompletion();

    expect(states[0]).toBe("checking");
    expect(states).toContain("downloading");
    expect(states).toContain("installing");
    expect(states[states.length - 1]).toBe("ready_to_restart");
    expect(completion?.state).toBe("ready_to_restart");
  });

  test("returns error when download fails", async () => {
    const updater = await loadUpdater();

    checkImpl = async () => ({
      available: true,
      version: "1.0.2",
      date: new Date().toISOString(),
      body: "test",
      downloadAndInstall: async () => {
        throw new Error("boom");
      },
    });

    updater.startBackgroundInstall();
    const completion = await updater.waitForInstallCompletion();

    expect(completion?.state).toBe("error");
    expect(completion?.errorCode).toBe("INSTALL_FAILED");
  });
});

describe("subscribeUpdateTask", () => {
  test("pushes initial snapshot and respects unsubscribe", async () => {
    const updater = await loadUpdater();

    checkImpl = async () => ({
      available: true,
      version: "1.0.3",
      date: new Date().toISOString(),
      body: "test",
      downloadAndInstall: async () => {},
    });

    const snapshots: string[] = [];
    const unsubscribe = updater.subscribeUpdateTask((s: any) => {
      snapshots.push(s.state);
    });

    expect(snapshots[0]).toBe("idle");
    unsubscribe();

    updater.startBackgroundInstall();
    await updater.waitForInstallCompletion();

    expect(snapshots).toEqual(["idle"]);
  });
});
