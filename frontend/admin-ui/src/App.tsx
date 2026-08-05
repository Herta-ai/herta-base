import { QueryClientProvider } from '@tanstack/react-query'
import { RouterProvider, createRouter } from '@tanstack/react-router'
import { useSelector } from '@tanstack/react-store'
import Theme, {
  ThemeProvider,
} from "@jetbrains/ring-ui-built/components/global/theme"
import { queryClient } from './lib/query-client'
import { routeTree } from './routeTree.gen'
import { appStore } from './store/app'

// 实例化 Router 并注入 queryClient (这样可以在 Router 的 loader 中预取数据)
const router = createRouter({
  routeTree,
  context: { queryClient }
})

export function App() {
  const dark = useSelector(appStore, (state) => state.dark)

  return (
    <ThemeProvider theme={dark ? Theme.DARK : Theme.LIGHT}>
      <QueryClientProvider client={queryClient}>
        <RouterProvider router={router} />
      </QueryClientProvider>
    </ThemeProvider>
  )
}
