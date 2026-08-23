import { createFileRoute, useNavigate } from '@tanstack/react-router';
import { useEffect } from 'react';

import { authStore } from '../store/auth';

export const Route = createFileRoute('/')({
  component: IndexRedirect,
});

function IndexRedirect() {
  const navigate = useNavigate();

  useEffect(() => {
    const isAuthed = authStore.state.isAuthenticated;
    if (isAuthed) {
      navigate({ to: '/collections' });
    } else {
      navigate({ to: '/login' });
    }
  }, [navigate]);

  return null;
}
