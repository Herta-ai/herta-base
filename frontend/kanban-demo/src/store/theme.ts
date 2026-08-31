import { Store } from '@tanstack/react-store';

export type ThemeMode = 'light' | 'dark' | 'system';

const STORAGE_KEY_THEME = 'hb_kanban_theme_mode';

function getInitialTheme(): ThemeMode {
  const saved = localStorage.getItem(STORAGE_KEY_THEME) as ThemeMode | null;
  if (saved && ['light', 'dark', 'system'].includes(saved)) {
    return saved;
  }
  return 'system';
}

export const themeStore = new Store<{ mode: ThemeMode }>({
  mode: getInitialTheme(),
});

export function setThemeMode(mode: ThemeMode) {
  themeStore.setState(() => ({ mode }));
  localStorage.setItem(STORAGE_KEY_THEME, mode);
  applyThemeToDocument(mode);
}

export function applyThemeToDocument(mode: ThemeMode) {
  const root = document.documentElement;
  const isDark =
    mode === 'dark' ||
    (mode === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);

  if (isDark) {
    root.classList.add('dark');
  } else {
    root.classList.remove('dark');
  }
}
