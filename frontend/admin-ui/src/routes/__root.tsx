import type { QueryClient } from '@tanstack/react-query';
import { createRootRouteWithContext, Outlet } from '@tanstack/react-router';

import { ThemedWrapper } from '../components/ThemedWrapper';

import './jetbrains-ide.css';

interface MyRouterContext {
  queryClient: QueryClient;
}

export const Route = createRootRouteWithContext<MyRouterContext>()({
  component: () => (
    <ThemedWrapper>
      <Outlet />
    </ThemedWrapper>
  ),
});
