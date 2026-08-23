import { useEffect } from 'react'
import { createFileRoute, Outlet, useNavigate, useLocation, Link } from '@tanstack/react-router'
import { useStore } from '@tanstack/react-store'
import { useQuery } from '@tanstack/react-query'
import Tooltip from '@jetbrains/ring-ui-built/components/tooltip/tooltip'
import { Directions } from '@jetbrains/ring-ui-built/components/popup/popup.consts'
import { appStore } from '../store/app'
import { authStore, clearAuthSession } from '../store/auth'
import { hbApi } from '../lib/api'
import { useI18n } from '../lib/i18n'
import { ThemedWrapper } from '../components/ThemedWrapper'
import './jetbrains-ide.css'

export const Route = createFileRoute('/_admin')({
  component: AdminLayout,
})

function AdminLayout() {
  const navigate = useNavigate()
  const location = useLocation()
  const { t, lang, setLang } = useI18n()

  const isDark = useStore(appStore, (state) => state.dark)
  const auth = useStore(authStore, (state) => state)

  // 认证守护
  useEffect(() => {
    if (!auth.isAuthenticated) {
      navigate({ to: '/login' })
    }
  }, [auth.isAuthenticated, navigate])

  // 获取集合列表
  const { data: collectionsRes, refetch: refetchCollections } = useQuery({
    queryKey: ['collections'],
    queryFn: async () => {
      const res = await hbApi.collections.list()
      return res.data.data || []
    },
    enabled: auth.isAuthenticated,
  })

  const collections = collectionsRes || []
  const baseCollections = collections.filter((c) => c.type === 'base')
  const authCollections = collections.filter((c) => c.type === 'auth')

  const toggleTheme = () => {
    appStore.setState((s) => ({ ...s, dark: !s.dark }))
  }

  const handleLogout = () => {
    if (window.confirm(t('auth.logout.confirm'))) {
      clearAuthSession()
      navigate({ to: '/login' })
    }
  }

  if (!auth.isAuthenticated) {
    return null
  }

  const currentPath = location.pathname

  const tooltipDirections = [
    Directions.RIGHT_CENTER,
    Directions.RIGHT_TOP,
    Directions.RIGHT_BOTTOM,
    Directions.BOTTOM_CENTER,
  ]

  return (
    <ThemedWrapper>
      <div className="jb-workspace">
        {/* 1. Header Bar */}
        <header className="jb-header">
          <div className="jb-header-left">
            <div className="jb-logo-badge">
              <div className="jb-logo-icon">HB</div>
              <span>HertaBase</span>
            </div>

            <div className="jb-project-name">
              <span>Admin Console</span>
              <span className="jb-branch-badge">
                <span className="i-ph:git-branch-bold text-11px mr-1" />
                v0.1.0
              </span>
            </div>

            {/* Quick Navigation Menu (纯文字，无左侧图标) */}
            <div className="jb-menu-bar">
              <Link
                to="/collections"
                className={`jb-menu-item ${currentPath.startsWith('/collections') ? 'active' : ''}`}
                style={{
                  textDecoration: 'none',
                  color: currentPath.startsWith('/collections') ? 'var(--jb-accent-blue)' : 'inherit',
                  fontWeight: currentPath.startsWith('/collections') ? 600 : 400,
                }}
              >
                {t('nav.collections')}
              </Link>
              <Link
                to="/logs"
                className={`jb-menu-item ${currentPath.startsWith('/logs') ? 'active' : ''}`}
                style={{
                  textDecoration: 'none',
                  color: currentPath.startsWith('/logs') ? 'var(--jb-accent-blue)' : 'inherit',
                  fontWeight: currentPath.startsWith('/logs') ? 600 : 400,
                }}
              >
                {t('nav.logs')}
              </Link>
              <Link
                to="/web"
                className={`jb-menu-item ${currentPath.startsWith('/web') ? 'active' : ''}`}
                style={{
                  textDecoration: 'none',
                  color: currentPath.startsWith('/web') ? 'var(--jb-accent-blue)' : 'inherit',
                  fontWeight: currentPath.startsWith('/web') ? 600 : 400,
                }}
              >
                Web Hosting
              </Link>
              <Link
                to="/settings"
                className={`jb-menu-item ${currentPath.startsWith('/settings') ? 'active' : ''}`}
                style={{
                  textDecoration: 'none',
                  color: currentPath.startsWith('/settings') ? 'var(--jb-accent-blue)' : 'inherit',
                  fontWeight: currentPath.startsWith('/settings') ? 600 : 400,
                }}
              >
                {t('nav.settings')}
              </Link>
            </div>
          </div>

          {/* Search Everywhere Box */}
          <div
            className="jb-search-everywhere"
            onClick={() => navigate({ to: '/collections' })}
          >
            <span className="i-ph:magnifying-glass-bold text-13px" />
            <span>Search Collections & Records</span>
            <span className="jb-kbd">Shift Shift</span>
          </div>

          <div className="jb-header-right">
            {/* Swagger OpenAPI Docs */}
            <a
              href="/swagger-ui"
              target="_blank"
              rel="noreferrer"
              title={t('nav.swagger')}
              style={{
                background: 'var(--jb-panel-bg)',
                border: '1px solid var(--jb-border)',
                color: 'var(--jb-accent-green)',
                borderRadius: 6,
                padding: '4px 10px',
                fontSize: 12,
                cursor: 'pointer',
                textDecoration: 'none',
                fontWeight: 600,
                display: 'flex',
                alignItems: 'center',
                gap: 5,
              }}
            >
              <span className="i-ph:book-open-text-bold text-13px" />
              <span>Swagger</span>
            </a>

            {/* Language Switcher */}
            <button
              onClick={() => setLang(lang === 'zh' ? 'en' : 'zh')}
              style={{
                background: 'var(--jb-panel-bg)',
                border: '1px solid var(--jb-border)',
                color: 'var(--jb-text)',
                borderRadius: 6,
                padding: '4px 10px',
                fontSize: 12,
                cursor: 'pointer',
                fontWeight: 500,
                display: 'flex',
                alignItems: 'center',
                gap: 5,
              }}
            >
              <span className="i-ph:translate-bold text-13px text-sky-400" />
              <span>{lang === 'zh' ? '中文' : 'EN'}</span>
            </button>

            {/* Theme Toggle Button */}
            <button
              onClick={toggleTheme}
              title={isDark ? t('app.theme.light') : t('app.theme.dark')}
              style={{
                background: 'var(--jb-panel-bg)',
                border: '1px solid var(--jb-border)',
                color: 'var(--jb-text)',
                borderRadius: 6,
                padding: '4px 8px',
                fontSize: 12,
                cursor: 'pointer',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
              }}
            >
              <span className={isDark ? 'i-ph:moon-stars-bold text-14px text-indigo-400' : 'i-ph:sun-dim-bold text-14px text-amber-400'} />
            </button>

            {/* Current Admin Capsule with embedded Logout Button */}
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 6,
                background: 'var(--jb-panel-bg)',
                padding: '3px 4px 3px 10px',
                borderRadius: 6,
                border: '1px solid var(--jb-border)',
                fontSize: 12,
              }}
            >
              <span className="i-ph:user-circle-gear-bold text-14px text-blue-500" />
              <span
                style={{
                  maxWidth: 140,
                  overflow: 'hidden',
                  textOverflow: 'ellipsis',
                  whiteSpace: 'nowrap',
                  fontWeight: 500,
                  color: 'var(--jb-text)',
                }}
                title={auth.admin?.email || 'Admin'}
              >
                {auth.admin?.email || 'Admin'}
              </span>
              <div style={{ width: 1, height: 14, backgroundColor: 'var(--jb-border)', margin: '0 2px' }} />
              <Tooltip
                title={t('auth.logout')}
                popupProps={{
                  directions: [Directions.BOTTOM_LEFT],
                  style: { zIndex: 99999 },
                  className: 'jb-tooltip-popup',
                }}
              >
                <button
                  onClick={handleLogout}
                  aria-label={t('auth.logout')}
                  style={{
                    background: 'transparent',
                    border: 'none',
                    color: 'var(--jb-text-muted)',
                    borderRadius: 4,
                    padding: '3px 5px',
                    fontSize: 12,
                    cursor: 'pointer',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    transition: 'color 0.15s, background-color 0.15s',
                  }}
                  onMouseEnter={(e) => {
                    e.currentTarget.style.color = '#ef4444'
                    e.currentTarget.style.backgroundColor = 'rgba(239, 68, 68, 0.12)'
                  }}
                  onMouseLeave={(e) => {
                    e.currentTarget.style.color = 'var(--jb-text-muted)'
                    e.currentTarget.style.backgroundColor = 'transparent'
                  }}
                >
                  <span className="i-ph:sign-out-bold text-13px" />
                </button>
              </Tooltip>
            </div>
          </div>
        </header>

        {/* 2. Main Workspace Layout */}
        <div className="jb-main-layout">
          {/* Left Side Tool Icon Bar (48px) - Using Ring UI Tooltip/Popup */}
          <aside className="jb-tool-sidebar">
            <Tooltip
              title={t('nav.collections')}
              popupProps={{ directions: tooltipDirections }}
            >
              <Link
                to="/collections"
                className={`jb-tool-icon ${currentPath.startsWith('/collections') ? 'active' : ''}`}
              >
                <span className="i-ph:database-bold" />
              </Link>
            </Tooltip>

            <Tooltip
              title={t('nav.logs')}
              popupProps={{ directions: tooltipDirections }}
            >
              <Link
                to="/logs"
                className={`jb-tool-icon ${currentPath.startsWith('/logs') ? 'active' : ''}`}
              >
                <span className="i-ph:terminal-window-bold" />
              </Link>
            </Tooltip>
            <Tooltip title="Web Hosting" popupProps={{ directions: tooltipDirections }}>
              <Link to="/web" className={`jb-tool-icon ${currentPath.startsWith('/web') ? 'active' : ''}`}>
                <span className="i-ph:globe-bold" />
              </Link>
            </Tooltip>

            <Tooltip
              title={t('nav.settings')}
              popupProps={{ directions: tooltipDirections }}
            >
              <Link
                to="/settings"
                className={`jb-tool-icon ${currentPath.startsWith('/settings') ? 'active' : ''}`}
              >
                <span className="i-ph:gear-six-bold" />
              </Link>
            </Tooltip>

            <div style={{ marginTop: 'auto', display: 'flex', flexDirection: 'column', gap: 8 }}>
              <Tooltip
                title={t('nav.swagger')}
                popupProps={{ directions: tooltipDirections }}
              >
                <a
                  href="/swagger-ui"
                  target="_blank"
                  rel="noreferrer"
                  className="jb-tool-icon"
                >
                  <span className="i-ph:file-code-bold" />
                </a>
              </Tooltip>
            </div>
          </aside>

          {/* Database Collections Tree Panel (240px) */}
          <aside className="jb-project-panel">
            {/* Panel Header */}
            <div className="jb-panel-title">
              <span style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                <span className="i-ph:tree-structure-bold text-13px" />
                <span>{t('nav.database')}</span>
              </span>
              <div style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
                <button
                  onClick={() => refetchCollections()}
                  title={t('app.refresh')}
                  style={{
                    background: 'transparent',
                    border: 'none',
                    color: 'var(--jb-text-muted)',
                    cursor: 'pointer',
                    display: 'flex',
                    alignItems: 'center',
                    padding: 2,
                  }}
                >
                  <span className="i-ph:arrow-clockwise-bold text-12px" />
                </button>
                <Link
                  to="/collections"
                  title={t('collections.create')}
                  style={{
                    background: 'var(--jb-accent-blue)',
                    color: '#fff',
                    borderRadius: 4,
                    width: 18,
                    height: 18,
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    fontSize: 11,
                    textDecoration: 'none',
                  }}
                >
                  <span className="i-ph:plus-bold" />
                </Link>
              </div>
            </div>

            {/* Tree List */}
            <div className="jb-tree-list">
              {/* Section: Base Collections */}
              <div
                style={{
                  padding: '6px 12px 4px',
                  fontSize: 11,
                  fontWeight: 700,
                  color: 'var(--jb-text-muted)',
                  textTransform: 'uppercase',
                  letterSpacing: 0.5,
                  display: 'flex',
                  alignItems: 'center',
                  gap: 6,
                }}
              >
                <span className="i-ph:folder-bold text-amber-400 text-13px" />
                <span>{t('nav.base_collections')}</span>
                <span style={{ marginLeft: 'auto', fontSize: 10 }}>({baseCollections.length})</span>
              </div>

              {baseCollections.length === 0 ? (
                <div style={{ padding: '6px 24px', fontSize: 12, color: 'var(--jb-text-muted)', fontStyle: 'italic' }}>
                  (无自定义数据表)
                </div>
              ) : (
                baseCollections.map((col) => {
                  const isActive = currentPath === `/collections/${col.name}`
                  return (
                    <Link
                      key={col.name}
                      to="/collections/$collectionName"
                      params={{ collectionName: col.name }}
                      className={`jb-tree-item ${isActive ? 'active' : ''}`}
                      style={{ paddingLeft: 24, textDecoration: 'none' }}
                    >
                      <span className="i-ph:table-bold text-sky-400 text-14px shrink-0" />
                      <span style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                        {col.name}
                      </span>
                      <span
                        style={{
                          fontSize: 10,
                          color: 'var(--jb-text-muted)',
                          background: 'var(--jb-border)',
                          padding: '1px 5px',
                          borderRadius: 6,
                        }}
                      >
                        {col.fields?.length || 0}f
                      </span>
                    </Link>
                  )
                })
              )}

              {/* Section: Auth Collections */}
              <div
                style={{
                  padding: '14px 12px 4px',
                  fontSize: 11,
                  fontWeight: 700,
                  color: 'var(--jb-text-muted)',
                  textTransform: 'uppercase',
                  letterSpacing: 0.5,
                  display: 'flex',
                  alignItems: 'center',
                  gap: 6,
                }}
              >
                <span className="i-ph:folder-bold text-purple-400 text-13px" />
                <span>{t('nav.auth_collections')}</span>
                <span style={{ marginLeft: 'auto', fontSize: 10 }}>({authCollections.length})</span>
              </div>

              {authCollections.length === 0 ? (
                <div style={{ padding: '6px 24px', fontSize: 12, color: 'var(--jb-text-muted)', fontStyle: 'italic' }}>
                  (无用户表)
                </div>
              ) : (
                authCollections.map((col) => {
                  const isActive = currentPath === `/collections/${col.name}`
                  return (
                    <Link
                      key={col.name}
                      to="/collections/$collectionName"
                      params={{ collectionName: col.name }}
                      className={`jb-tree-item ${isActive ? 'active' : ''}`}
                      style={{ paddingLeft: 24, textDecoration: 'none' }}
                    >
                      <span className="i-ph:shield-check-bold text-purple-400 text-14px shrink-0" />
                      <span style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                        {col.name}
                      </span>
                      <span
                        style={{
                          fontSize: 10,
                          color: 'var(--jb-text-muted)',
                          background: 'var(--jb-border)',
                          padding: '1px 5px',
                          borderRadius: 6,
                        }}
                      >
                        auth
                      </span>
                    </Link>
                  )
                })
              )}
            </div>

            {/* Quick Panel Footer Navigation */}
            <div
              style={{
                padding: '8px 10px',
                borderTop: '1px solid var(--jb-border)',
                background: 'var(--jb-header-bg)',
                display: 'flex',
                flexDirection: 'column',
                gap: 4,
              }}
            >
              <Link
                to="/collections"
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 8,
                  fontSize: 12,
                  color: 'var(--jb-text)',
                  textDecoration: 'none',
                  padding: '4px 6px',
                  borderRadius: 4,
                }}
              >
                <span className="i-ph:wrench-bold text-amber-500" />
                <span>{t('collections.title')}</span>
              </Link>
              <Link
                to="/settings"
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 8,
                  fontSize: 12,
                  color: 'var(--jb-text)',
                  textDecoration: 'none',
                  padding: '4px 6px',
                  borderRadius: 4,
                }}
              >
                <span className="i-ph:terminal-bold text-emerald-500" />
                <span>{t('settings.tab.sql')}</span>
              </Link>
            </div>
          </aside>

          {/* Center Main Editor / Viewport */}
          <main className="jb-editor-container">
            <Outlet />
          </main>
        </div>

        {/* 3. Bottom Status Bar */}
        <footer className="jb-status-bar">
          <div className="jb-status-left">
            <span className="jb-status-item">
              <span className="i-ph:git-branch-bold text-11px text-blue-400" />
              <span>main</span>
            </span>

            <span className="jb-status-item">
              <span className="i-ph:database-bold text-11px text-emerald-400" />
              <span>{t('status.db')}: <strong>SurrealKV / Memory</strong></span>
            </span>

            <span className="jb-status-item">
              <span className="i-ph:cpu-bold text-11px text-purple-400" />
              <span>{t('status.engine')}: <strong>Rust Core + Boa JS VM</strong></span>
            </span>

            <span className="jb-status-item">
              <span className="i-ph:broadcast-bold text-11px text-emerald-400" />
              <span>{t('status.realtime')}: <strong style={{ color: 'var(--jb-accent-green)' }}>SSE Ready</strong></span>
            </span>
          </div>

          <div className="jb-status-right">
            <span className="jb-status-item">
              <span className="i-ph:check-circle-bold text-11px text-emerald-400" />
              <span>0 errors</span>
            </span>
            <span className="jb-status-item">UTF-8</span>
            <span className="jb-status-item">CRLF</span>
            <span className="jb-status-item">Admin UI 2.0</span>
          </div>
        </footer>
      </div>
    </ThemedWrapper>
  )
}
