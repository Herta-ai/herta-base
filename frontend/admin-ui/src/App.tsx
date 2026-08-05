import { QueryClientProvider } from '@tanstack/react-query'
import { RouterProvider, createRouter } from '@tanstack/react-router'
import { useSelector } from '@tanstack/react-store'
import Theme, {
  ThemeProvider,
} from "@jetbrains/ring-ui-built/components/global/theme"
import { queryClient } from './lib/query-client'
import { routeTree } from './routeTree.gen'
import { appStore } from './store/app'
import { I18nProvider } from './lib/i18n'

const router = createRouter({
  routeTree,
  context: { queryClient },
})

export function App() {
  const dark = useSelector(appStore, (state) => state.dark)

  return (
    <I18nProvider>
      <ThemeProvider theme={dark ? Theme.DARK : Theme.LIGHT}>
        <QueryClientProvider client={queryClient}>
          <RouterProvider router={router} />
        </QueryClientProvider>
      </ThemeProvider>
    </I18nProvider>
  )
}
