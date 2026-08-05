import { Store } from '@tanstack/react-store'

const darkMatcher = window.matchMedia("(prefers-color-scheme: dark)");

export interface AppState {
  dark: boolean
}

export const appStore = new Store<AppState>({
  dark: darkMatcher.matches,
});
