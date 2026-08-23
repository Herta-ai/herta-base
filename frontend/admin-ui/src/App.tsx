import Theme, { ThemeProvider } from '@jetbrains/ring-ui-built/components/global/theme';
import { QueryClientProvider } from '@tanstack/react-query';
import { RouterProvider, createRouter } from '@tanstack/react-router';
import { useSelector } from '@tanstack/react-store';

import { I18nProvider } from './lib/i18n';
import { queryClient } from './lib/query-client';
import { routeTree } from './routeTree.gen';
import { appStore } from './store/app';

const router = createRouter({
  routeTree,
  basepath: '/webui',
  context: { queryClient },
});

export function App() {
  const dark = useSelector(appStore, (state) => state.dark);

  return (
    <I18nProvider>
      <ThemeProvider theme={dark ? Theme.DARK : Theme.LIGHT}>
        <QueryClientProvider client={queryClient}>
          <RouterProvider router={router} />
        </QueryClientProvider>
      </ThemeProvider>
    </I18nProvider>
  );
}
