import { useState, useRef, useEffect } from 'react'
import { createFileRoute } from '@tanstack/react-router'
import Button from '@jetbrains/ring-ui-built/components/button/button'
import { useI18n } from '../../lib/i18n'

export const Route = createFileRoute('/_admin/logs')({
  component: SystemLogsPage,
})

interface LogEntry {
  id: string
  timestamp: string
  level: 'DEBUG' | 'INFO' | 'WARN' | 'ERROR' | 'FATAL'
  target: string
  message: string
  context?: Record<string, unknown>
}

const INITIAL_LOGS: LogEntry[] = [
  {
    id: 'log-1',
    timestamp: new Date(Date.now() - 60000).toISOString(),
    level: 'INFO',
    target: 'herta_server::init',
    message: 'HertaBase server initializing with memory engine (SurrealDB mem://)',
  },
  {
    id: 'log-2',
    timestamp: new Date(Date.now() - 55000).toISOString(),
    level: 'INFO',
    target: 'herta_auth::bootstrap',
    message: 'Superuser bootstrap check passed. Admin account ready: admin@example.com',
  },
  {
    id: 'log-3',
    timestamp: new Date(Date.now() - 48000).toISOString(),
    level: 'INFO',
    target: 'herta_hooks::boa',
    message: 'Boa JavaScript Runtime Sandbox initialized successfully.',
  },
  {
    id: 'log-4',
    timestamp: new Date(Date.now() - 30000).toISOString(),
    level: 'DEBUG',
    target: 'herta_core::schema',
    message: 'System collections loaded: _admins, _collections, _hooks',
  },
  {
    id: 'log-5',
    timestamp: new Date(Date.now() - 10000).toISOString(),
    level: 'INFO',
    target: 'herta_realtime::sse',
    message: 'SSE Broadcast Hub listening for event subscriptions on /api/realtime/*',
  },
]

function SystemLogsPage() {
  const { t } = useI18n()
  const [logs, setLogs] = useState<LogEntry[]>(INITIAL_LOGS)
  const [selectedLevel, setSelectedLevel] = useState<string>('ALL')
  const [keyword, setKeyword] = useState('')
  const [autoScroll, setAutoScroll] = useState(true)
  const [selectedLog, setSelectedLog] = useState<LogEntry | null>(null)

  const logEndRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (autoScroll) {
      logEndRef.current?.scrollIntoView({ behavior: 'smooth' })
    }
  }, [logs, autoScroll])

  const filteredLogs = logs.filter((log) => {
    if (selectedLevel !== 'ALL' && log.level !== selectedLevel) {
      return false
    }
    if (keyword.trim() !== '') {
      const matchKey = keyword.toLowerCase()
      return (
        log.message.toLowerCase().includes(matchKey) ||
        log.target.toLowerCase().includes(matchKey)
      )
    }
    return true
  })

  const handleClear = () => {
    setLogs([])
  }

  const handleCopyLogs = () => {
    const text = filteredLogs
      .map((l) => `[${l.timestamp}] [${l.level}] [${l.target}] ${l.message}`)
      .join('\n')
    navigator.clipboard.writeText(text)
    alert(t('app.copied'))
  }

  const getLevelColor = (lvl: string) => {
    switch (lvl) {
      case 'DEBUG':
        return '#8752a3'
      case 'INFO':
        return '#3574f0'
      case 'WARN':
        return '#d67929'
      case 'ERROR':
      case 'FATAL':
        return '#e53935'
      default:
        return 'var(--jb-text-muted)'
    }
  }

  return (
    <div style={{ flex: 1, display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden' }}>
      {/* Tab Header */}
      <div className="jb-editor-tabs" style={{ justifyContent: 'space-between', paddingRight: 10 }}>
        <div style={{ display: 'flex' }}>
          <div className="jb-editor-tab active">
            <span className="i-ph:terminal-window-bold text-amber-500 text-13px" />
            <span>{t('logs.title')}</span>
            <span style={{ fontSize: 10, color: 'var(--jb-text-muted)', background: 'var(--jb-border)', padding: '1px 5px', borderRadius: 4 }}>
              {logs.length}
            </span>
          </div>
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <Button onClick={handleCopyLogs} style={{ height: 26, fontSize: 11, display: 'flex', alignItems: 'center', gap: 4 }}>
            <span className="i-ph:copy-bold text-11px" />
            <span>{t('app.copy')}</span>
          </Button>
          <Button onClick={handleClear} style={{ height: 26, fontSize: 11, display: 'flex', alignItems: 'center', gap: 4 }}>
            <span className="i-ph:trash-bold text-11px" />
            <span>{t('logs.clear')}</span>
          </Button>
        </div>
      </div>

      {/* Breadcrumbs */}
      <div className="jb-breadcrumbs">
        <span>HertaBase</span>
        <span className="jb-breadcrumb-sep">›</span>
        <span>Monitoring</span>
        <span className="jb-breadcrumb-sep">›</span>
        <span style={{ color: 'var(--jb-accent-blue)', fontWeight: 500 }}>System Logs</span>
      </div>

      {/* Filters Toolbar */}
      <div
        style={{
          padding: '8px 16px',
          background: 'var(--jb-header-bg)',
          borderBottom: '1px solid var(--jb-border)',
          display: 'flex',
          gap: 12,
          alignItems: 'center',
          flexWrap: 'wrap',
        }}
      >
        {/* Level Selector Pills */}
        <div style={{ display: 'flex', gap: 4 }}>
          {['ALL', 'DEBUG', 'INFO', 'WARN', 'ERROR'].map((lvl) => {
            const isActive = selectedLevel === lvl
            return (
              <button
                key={lvl}
                onClick={() => setSelectedLevel(lvl)}
                style={{
                  background: isActive ? 'var(--jb-active-item)' : 'var(--jb-panel-bg)',
                  border: '1px solid var(--jb-border)',
                  color: isActive ? 'var(--jb-accent-blue)' : 'var(--jb-text)',
                  fontWeight: isActive ? 700 : 400,
                  borderRadius: 4,
                  padding: '3px 8px',
                  fontSize: 11,
                  cursor: 'pointer',
                  display: 'flex',
                  alignItems: 'center',
                  gap: 4,
                }}
              >
                {lvl === 'DEBUG' && <span className="i-ph:bug-bold text-purple-400" />}
                {lvl === 'INFO' && <span className="i-ph:info-bold text-blue-400" />}
                {lvl === 'WARN' && <span className="i-ph:warning-bold text-amber-400" />}
                {lvl === 'ERROR' && <span className="i-ph:x-circle-bold text-rose-500" />}
                <span>{lvl === 'ALL' ? t('logs.level.all') : lvl}</span>
              </button>
            )
          })}
        </div>

        {/* Search Keyword */}
        <div style={{ flex: 1, minWidth: 200, display: 'flex', alignItems: 'center', position: 'relative' }}>
          <span className="i-ph:magnifying-glass-bold text-13px text-zinc-400 absolute left-3" />
          <input
            type="text"
            placeholder={t('logs.search_placeholder')}
            value={keyword}
            onChange={(e) => setKeyword(e.target.value)}
            style={{
              width: '100%',
              padding: '4px 8px 4px 28px',
              borderRadius: 4,
              border: '1px solid var(--jb-border)',
              backgroundColor: 'var(--jb-editor-bg)',
              color: 'var(--jb-text)',
              fontSize: 12,
              boxSizing: 'border-box',
            }}
          />
        </div>

        {/* Auto Scroll */}
        <label style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 11, cursor: 'pointer', color: 'var(--jb-text-muted)' }}>
          <input
            type="checkbox"
            checked={autoScroll}
            onChange={(e) => setAutoScroll(e.target.checked)}
          />
          <span className="i-ph:arrows-down-up-bold text-12px" />
          <span>{t('logs.auto_scroll')}</span>
        </label>
      </div>

      {/* Terminal Log Console */}
      <div
        style={{
          flex: 1,
          backgroundColor: 'var(--jb-dock-bg)',
          color: 'var(--jb-console-text)',
          fontFamily: 'JetBrains Mono, Menlo, Consolas, monospace',
          fontSize: 12,
          padding: '12px 16px',
          overflowY: 'auto',
          lineHeight: 1.6,
        }}
      >
        {filteredLogs.length === 0 ? (
          <div style={{ padding: 40, textAlign: 'center', color: 'var(--jb-text-muted)' }}>
            <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 8 }}>
              <span className="i-ph:scroll-bold text-32px text-zinc-500" />
              <span>{t('logs.empty')}</span>
            </div>
          </div>
        ) : (
          filteredLogs.map((log) => {
            const lvlColor = getLevelColor(log.level)
            return (
              <div
                key={log.id}
                onClick={() => setSelectedLog(log)}
                style={{
                  display: 'flex',
                  gap: 12,
                  padding: '2px 6px',
                  borderRadius: 4,
                  cursor: 'pointer',
                  transition: 'background-color 0.1s',
                  alignItems: 'baseline',
                }}
                className="hover:bg-[var(--jb-hover-item)]"
              >
                <span style={{ color: 'var(--jb-text-muted)', fontSize: 11, flexShrink: 0 }}>
                  {new Date(log.timestamp).toLocaleTimeString()}
                </span>

                <span
                  style={{
                    color: lvlColor,
                    fontWeight: 700,
                    fontSize: 11,
                    width: 52,
                    textAlign: 'center',
                    background: `${lvlColor}18`,
                    borderRadius: 3,
                    padding: '0 4px',
                  }}
                >
                  {log.level}
                </span>

                <span style={{ color: 'var(--jb-accent-purple)', fontSize: 11, width: 140, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                  {log.target}
                </span>

                <span style={{ flex: 1, color: 'var(--jb-text)', wordBreak: 'break-all' }}>
                  {log.message}
                </span>
              </div>
            )
          })
        )}
        <div ref={logEndRef} />
      </div>

      {/* Log Detail Drawer Modal */}
      {selectedLog && (
        <div
          style={{
            position: 'fixed',
            inset: 0,
            backgroundColor: 'rgba(0,0,0,0.55)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            zIndex: 1000,
          }}
        >
          <div
            style={{
              width: 560,
              background: 'var(--jb-panel-bg)',
              border: '1px solid var(--jb-border)',
              borderRadius: 8,
              boxShadow: 'var(--jb-shadow)',
              display: 'flex',
              flexDirection: 'column',
              overflow: 'hidden',
            }}
          >
            <div style={{ padding: '10px 16px', background: 'var(--jb-header-bg)', borderBottom: '1px solid var(--jb-border)', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <span style={{ fontSize: 13, fontWeight: 700, display: 'flex', alignItems: 'center', gap: 6 }}>
                <span className="i-ph:terminal-bold text-sky-400 text-14px" />
                <span>Log Inspector</span>
              </span>
              <button onClick={() => setSelectedLog(null)} style={{ background: 'transparent', border: 'none', color: 'var(--jb-text-muted)', cursor: 'pointer', display: 'flex', padding: 2 }}>
                <span className="i-ph:x-bold text-14px" />
              </button>
            </div>
            <div style={{ padding: 16, display: 'flex', flexDirection: 'column', gap: 10, fontSize: 12 }}>
              <div><strong>Timestamp:</strong> {selectedLog.timestamp}</div>
              <div><strong>Level:</strong> <span style={{ color: getLevelColor(selectedLog.level), fontWeight: 700 }}>{selectedLog.level}</span></div>
              <div><strong>Target Module:</strong> <code>{selectedLog.target}</code></div>
              <div><strong>Message:</strong></div>
              <pre style={{ margin: 0, fontFamily: 'JetBrains Mono, monospace', background: 'var(--jb-dock-bg)', padding: 10, borderRadius: 4, border: '1px solid var(--jb-border)', wordBreak: 'break-all', whiteSpace: 'pre-wrap' }}>
                {selectedLog.message}
              </pre>
            </div>
            <div style={{ padding: '8px 16px', background: 'var(--jb-header-bg)', borderTop: '1px solid var(--jb-border)', textAlign: 'right' }}>
              <Button onClick={() => setSelectedLog(null)} style={{ height: 28 }}>{t('app.close')}</Button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
