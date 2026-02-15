import { createStore } from "solid-js/store";

export interface SettingsStore {
  signalServerUrl: string;
  defaultSaveDir: string;
  theme: "dark" | "light" | "system";
}

const stored = {
  signalServerUrl:
    localStorage.getItem("relay:signalServerUrl") || "ws://localhost:8080",
  defaultSaveDir: localStorage.getItem("relay:defaultSaveDir") || "",
  theme: (localStorage.getItem("relay:theme") as "dark" | "light" | "system") || "system",
};

export const [settings, setSettings] = createStore<SettingsStore>(stored);

export function updateSetting<K extends keyof SettingsStore>(
  key: K,
  value: SettingsStore[K]
) {
  setSettings(key, value);
  localStorage.setItem(`relay:${key}`, String(value));

  // Apply theme when it changes
  if (key === "theme") {
    applyTheme(value as "dark" | "light" | "system");
  }
}

export function applyTheme(theme: "dark" | "light" | "system") {
  const isDark = theme === "dark" ||
    (theme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);

  document.documentElement.classList.toggle("dark", isDark);
}

// Initialize theme on module load
applyTheme(stored.theme);
