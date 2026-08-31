import { useEffect } from 'react';
import { useSelector } from '@tanstack/react-store';
import { applyThemeToDocument, setThemeMode, themeStore, type ThemeMode } from '../store/theme';

export function useTheme() {
  const mode = useSelector(themeStore, (s) => s.mode);

  useEffect(() => {
    applyThemeToDocument(mode);

    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    const handleChange = () => {
      if (themeStore.state.mode === 'system') {
        applyThemeToDocument('system');
      }
    };

    mediaQuery.addEventListener('change', handleChange);
    return () => mediaQuery.removeEventListener('change', handleChange);
  }, [mode]);

  return {
    theme: mode,
    setTheme: setThemeMode,
    isDark: mode === 'dark' || (mode === 'system' && typeof window !== 'undefined' && window.matchMedia('(prefers-color-scheme: dark)').matches),
  };
}
