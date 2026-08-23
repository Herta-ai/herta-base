import { createFileRoute, redirect } from '@tanstack/react-router';

import { authStore, getAccessToken } from '../store/auth';

export const Route = createFileRoute('/')({
  beforeLoad: () => {
    const isAuthed = authStore.state.isAuthenticated || Boolean(getAccessToken());
    if (isAuthed) {
      throw redirect({ to: '/collections' });
    } else {
      throw redirect({ to: '/login' });
    }
  },
  component: () => null,
});
