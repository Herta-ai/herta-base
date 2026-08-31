import React, { useEffect, useState } from 'react';
import { QueryClientProvider } from '@tanstack/react-query';
import { useSelector } from '@tanstack/react-store';
import { Toaster } from 'sonner';
import { Loader2 } from 'lucide-react';
import { queryClient } from './lib/query-client';
import { checkDatabaseInitialized, type CheckResult } from './lib/init-checker';
import { authStore } from './store/auth';
import { useTheme } from './hooks/use-theme';
import { SetupPage } from './views/setup-page';
import { LoginPage } from './views/login-page';
import { MainView } from './views/main-view';

export function KanbanApp() {
  const { isDark } = useTheme();
  const { isAuthenticated } = useSelector(authStore, (s) => s);

  const [initCheck, setInitCheck] = useState<CheckResult | null>(null);
  const [checking, setChecking] = useState(true);
  const [manualView, setManualView] = useState<'setup' | null>(null);

  const verifyInitialization = async () => {
    setChecking(true);
    try {
      const res = await checkDatabaseInitialized();
      setInitCheck(res);
    } finally {
      setChecking(false);
    }
  };

  useEffect(() => {
    void verifyInitialization();
  }, []);

  if (checking) {
    return (
      <div className="min-h-screen flex flex-col items-center justify-center space-y-3 bg-background">
        <Loader2 className="h-8 w-8 animate-spin text-primary" />
        <p className="text-xs text-muted-foreground font-mono">
          正在探测 HertaBase 数据库与 Schema 状态...
        </p>
      </div>
    );
  }

  // If not initialized or unreachable, or user manually opened setup -> show SetupPage
  if (manualView === 'setup' || initCheck?.status !== 'ready') {
    return (
      <>
        <SetupPage
          onInitialized={async () => {
            setManualView(null);
            await verifyInitialization();
          }}
        />
        <Toaster richColors position="top-right" theme={isDark ? 'dark' : 'light'} />
      </>
    );
  }

  // If not logged in -> show LoginPage
  if (!isAuthenticated) {
    return (
      <>
        <LoginPage
          onSuccess={() => {
            // Logged in successfully
          }}
          onGoToSetup={() => setManualView('setup')}
        />
        <Toaster richColors position="top-right" theme={isDark ? 'dark' : 'light'} />
      </>
    );
  }

  // Logged in & Initialized -> show MainView (Board / Table)
  return (
    <>
      <MainView />
      <Toaster richColors position="top-right" theme={isDark ? 'dark' : 'light'} />
    </>
  );
}

export function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <KanbanApp />
    </QueryClientProvider>
  );
}
