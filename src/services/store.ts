import { Store } from "@tauri-apps/plugin-store";
import { isTauri } from "./api";

// Initialize the store. "settings.json" will be created in the app's appData directory.
// We use a singleton pattern to ensure we're always using the same store instance.
export const settingsStore = isTauri() ? new Store("settings.json") : null;

// Helper to save immediately after set
export async function saveSetting<T>(key: string, value: T): Promise<void> {
  if (!settingsStore) {
    // Fallback for web mode: localStorage
    try {
      localStorage.setItem(key, JSON.stringify(value));
    } catch (e) {
      console.error("Failed to save to localStorage", e);
    }
    return;
  }

  try {
    await settingsStore.set(key, value);
    await settingsStore.save();
  } catch (err) {
    console.error(`Failed to save setting ${key}:`, err);
  }
}

// Helper to get with fallback
export async function getSetting<T>(key: string, defaultValue: T): Promise<T> {
  if (!settingsStore) {
    // Fallback for web mode
    try {
      const item = localStorage.getItem(key);
      return item ? JSON.parse(item) : defaultValue;
    } catch {
      return defaultValue;
    }
  }

  try {
    const val = await settingsStore.get<T>(key);
    return val !== null && val !== undefined ? val : defaultValue;
  } catch (err) {
    console.error(`Failed to get setting ${key}:`, err);
    return defaultValue;
  }
}
