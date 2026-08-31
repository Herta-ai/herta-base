import Button from '@jetbrains/ring-ui-built/components/button/button';
import { createFileRoute, useNavigate } from '@tanstack/react-router';
import { useStore } from '@tanstack/react-store';
import { useState } from 'react';

import { ThemedWrapper } from '../components/ThemedWrapper';
import { hbApi, isHertaError } from '../lib/api';
import { useI18n } from '../lib/i18n';
import { appStore } from '../store/app';
import { setAuthSession } from '../store/auth';

import './jetbrains-ide.css';

export const Route = createFileRoute('/login')({
  component: AdminLoginPage,
});

function AdminLoginPage() {
  const navigate = useNavigate();
  const { t, lang, setLang } = useI18n();
  const isDark = useStore(appStore, (state) => state.dark);

  const [email, setEmail] = useState('admin@herta.ai');
  const [password, setPassword] = useState('password123');
  const [loading, setLoading] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const handleLogin = async (e: React.FormEvent) => {
    e.preventDefault();

    if (!email || !password) {
      setErrorMessage(t('auth.login.failed'));
      return;
    }

    setLoading(true);
    setErrorMessage(null);

    try {
      const session = await hbApi.auth.login({ email: email.trim(), password });

      if (session?.accessToken && session?.user) {
        setAuthSession(session);
        navigate({ to: '/collections' });
      } else {
        setErrorMessage(t('auth.login.failed'));
      }
    } catch (err: unknown) {
      const msg = isHertaError(err) ? err.message : ((err as Error)?.message || t('auth.login.failed'));
      setErrorMessage(msg);
    } finally {
      setLoading(false);
    }
  };

  const toggleTheme = () => {
    appStore.setState((s) => ({ ...s, dark: !s.dark }));
  };

  return (
    <ThemedWrapper>
      <div
        style={{
          width: '100vw',
          height: '100vh',
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          justifyContent: 'center',
          backgroundColor: 'var(--jb-bg)',
          color: 'var(--jb-text)',
          position: 'relative',
          userSelect: 'none',
        }}
      >
        {/* Top Utility Bar */}
        <div
          style={{
            position: 'absolute',
            top: 20,
            right: 28,
            display: 'flex',
            gap: 12,
            alignItems: 'center',
          }}
        >
          {/* Language Switch */}
          <button
            onClick={() => setLang(lang === 'zh' ? 'en' : 'zh')}
            style={{
              background: 'var(--jb-panel-bg)',
              border: '1px solid var(--jb-border)',
              color: 'var(--jb-text)',
              borderRadius: 6,
              padding: '5px 12px',
              fontSize: 12,
              cursor: 'pointer',
              fontWeight: 500,
              display: 'flex',
              alignItems: 'center',
              gap: 6,
              transition: 'all 0.15s',
            }}
          >
            <span className="i-ph:translate-bold text-13px text-sky-400" />
            <span>{lang === 'zh' ? '简体中文' : 'English'}</span>
          </button>

          {/* Theme Toggle Button */}
          <button
            onClick={toggleTheme}
            style={{
              background: 'var(--jb-panel-bg)',
              border: '1px solid var(--jb-border)',
              color: 'var(--jb-text)',
              borderRadius: 6,
              padding: '6px 12px',
              fontSize: 12,
              cursor: 'pointer',
              display: 'flex',
              alignItems: 'center',
              gap: 6,
              fontWeight: 500,
              transition: 'all 0.15s',
            }}
          >
            <span
              className={
                isDark
                  ? 'i-ph:moon-stars-bold text-14px text-indigo-400'
                  : 'i-ph:sun-dim-bold text-14px text-amber-400'
              }
            />
            <span>{isDark ? t('app.theme.dark') : t('app.theme.light')}</span>
          </button>
        </div>

        {/* Main IDE Splash Dialog */}
        <div
          style={{
            width: 460,
            background: 'var(--jb-panel-bg)',
            border: '1px solid var(--jb-border)',
            borderRadius: 10,
            boxShadow: 'var(--jb-shadow)',
            padding: '36px 32px',
            display: 'flex',
            flexDirection: 'column',
            boxSizing: 'border-box',
          }}
        >
          {/* Header Logo & Title */}
          <div style={{ display: 'flex', alignItems: 'center', gap: 14, marginBottom: 24 }}>
            <div
              className="jb-logo-icon"
              style={{ width: 44, height: 44, fontSize: 18, borderRadius: 8 }}
            >
              HB
            </div>
            <div>
              <div
                style={{
                  fontSize: 18,
                  fontWeight: 700,
                  color: 'var(--jb-text-heading)',
                  display: 'flex',
                  alignItems: 'center',
                  gap: 8,
                }}
              >
                <span>{t('auth.login.title')}</span>
                <span className="jb-branch-badge">
                  <span className="i-ph:shield-star-bold text-11px mr-1" />
                  Admin
                </span>
              </div>
              <div style={{ fontSize: 12, color: 'var(--jb-text-muted)', marginTop: 2 }}>
                {t('app.tagline')}
              </div>
            </div>
          </div>

          {/* Error Alert */}
          {errorMessage && (
            <div
              style={{
                backgroundColor: 'rgba(229, 57, 53, 0.12)',
                border: '1px solid #e53935',
                color: '#e53935',
                padding: '8px 12px',
                borderRadius: 6,
                fontSize: 12,
                marginBottom: 18,
                display: 'flex',
                alignItems: 'center',
                gap: 8,
              }}
            >
              <span className="i-ph:warning-circle-bold text-15px shrink-0" />
              <span>{errorMessage}</span>
            </div>
          )}

          {/* Form */}
          <form
            onSubmit={handleLogin}
            style={{ display: 'flex', flexDirection: 'column', gap: 16 }}
          >
            <div>
              <label
                style={{
                  fontSize: 12,
                  fontWeight: 600,
                  color: 'var(--jb-text-muted)',
                  display: 'flex',
                  alignItems: 'center',
                  gap: 6,
                  marginBottom: 6,
                }}
              >
                <span className="i-ph:envelope-simple-bold text-13px" />
                <span>{t('auth.email')}</span>
              </label>
              <input
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder={t('auth.email.placeholder')}
                autoComplete="username"
                required
                style={{
                  width: '100%',
                  padding: '8px 12px',
                  borderRadius: 6,
                  border: '1px solid var(--jb-border)',
                  backgroundColor: 'var(--jb-editor-bg)',
                  color: 'var(--jb-text)',
                  fontSize: 13,
                  outline: 'none',
                  boxSizing: 'border-box',
                }}
              />
            </div>

            <div>
              <label
                style={{
                  fontSize: 12,
                  fontWeight: 600,
                  color: 'var(--jb-text-muted)',
                  display: 'flex',
                  alignItems: 'center',
                  gap: 6,
                  marginBottom: 6,
                }}
              >
                <span className="i-ph:lock-key-bold text-13px" />
                <span>{t('auth.password')}</span>
              </label>
              <input
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder={t('auth.password.placeholder')}
                autoComplete="current-password"
                required
                style={{
                  width: '100%',
                  padding: '8px 12px',
                  borderRadius: 6,
                  border: '1px solid var(--jb-border)',
                  backgroundColor: 'var(--jb-editor-bg)',
                  color: 'var(--jb-text)',
                  fontSize: 13,
                  outline: 'none',
                  boxSizing: 'border-box',
                }}
              />
            </div>

            <div style={{ marginTop: 8 }}>
              <Button
                primary
                disabled={loading}
                type="submit"
                style={{
                  width: '100%',
                  height: 38,
                  fontSize: 14,
                  fontWeight: 600,
                  backgroundColor: 'var(--jb-accent-blue)',
                  borderColor: 'var(--jb-accent-blue)',
                  color: '#fff',
                  cursor: loading ? 'not-allowed' : 'pointer',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  gap: 6,
                }}
              >
                {loading ? (
                  <>
                    <span className="i-ph:spinner-gap-bold animate-spin text-15px" />
                    <span>{t('auth.logging_in')}</span>
                  </>
                ) : (
                  <>
                    <span className="i-ph:sign-in-bold text-15px" />
                    <span>{t('auth.login.btn')}</span>
                  </>
                )}
              </Button>
            </div>
          </form>

          {/* Quick Debug Help */}
          <div
            style={{
              marginTop: 24,
              paddingTop: 16,
              borderTop: '1px solid var(--jb-border)',
              fontSize: 11,
              color: 'var(--jb-text-muted)',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
            }}
          >
            <span style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
              <span className="i-ph:lightbulb-bold text-amber-400 text-13px" />
              <span>默认超级管理员凭据:</span>
            </span>
            <code style={{ background: 'rgba(0,0,0,0.1)', padding: '2px 6px', borderRadius: 4 }}>
              admin@example.com
            </code>
          </div>
        </div>

        {/* Footer Version */}
        <div
          style={{
            position: 'absolute',
            bottom: 16,
            fontSize: 12,
            color: 'var(--jb-text-muted)',
            display: 'flex',
            alignItems: 'center',
            gap: 10,
          }}
        >
          <span style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
            <span className="i-ph:cpu-bold text-13px" />
            <span>HertaBase Microkernel v0.1.0</span>
          </span>
          <span>•</span>
          <span style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
            <span className="i-ph:database-bold text-13px" />
            <span>SurrealDB & Rust Core</span>
          </span>
        </div>
      </div>
    </ThemedWrapper>
  );
}
