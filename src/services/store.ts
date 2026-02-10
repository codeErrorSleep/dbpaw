import { Store } from "@tauri-apps/plugin-store";
import { isTauri } from "./api";

// Initialize the store lazily. "settings.json" will be created in the app's appData directory.
// We use a singleton pattern to ensure we're always using the same store instance.
let storePromise: Promise<Store> | null = null;

async function getStore(): Promise<Store | null> {
  if (!isTauri()) return null;

  if (!storePromise) {
    storePromise = Store.load("settings.json");
  }

  try {
    return await storePromise;
  } catch (e) {
    console.error("Failed to load store:", e);
    return null;
  }
}

// Helper to save immediately after set
export async function saveSetting<T>(key: string, value: T): Promise<void> {
  const store = await getStore();

  if (!store) {
    // Fallback for web mode: localStorage
    try {
      localStorage.setItem(key, JSON.stringify(value));
    } catch (e) {
      console.error("Failed to save to localStorage", e);
    }
    return;
  }

  try {
    await store.set(key, value);
    await store.save();
  } catch (err) {
    console.error(`Failed to save setting ${key}:`, err);
  }
}

// Helper to get with fallback
export async function getSetting<T>(key: string, defaultValue: T): Promise<T> {
  const store = await getStore();

  if (!store) {
    // Fallback for web mode
    try {
      const item = localStorage.getItem(key);
      return item ? JSON.parse(item) : defaultValue;
    } catch (e) {
      console.warn(`Failed to parse setting ${key} from localStorage:`, e);
      return defaultValue;
    }
  }

  try {
    const val = await store.get<T>(key);
    return val !== null && val !== undefined ? val : defaultValue;
  } catch (err) {
    console.error(`Failed to get setting ${key}:`, err);
    return defaultValue;
  }
}
