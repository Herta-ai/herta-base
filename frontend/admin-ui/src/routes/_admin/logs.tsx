import { useState, useRef, useEffect, useMemo } from 'react'
import { createFileRoute } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'
import Button from '@jetbrains/ring-ui-built/components/button/button'
import Loader from '@jetbrains/ring-ui-built/components/loader/loader'
import Tooltip from '@jetbrains/ring-ui-built/components/tooltip/tooltip'
import { hbApi, type LogEntry, type LogQueryParams } from '../../lib/api'
import { useI18n } from '../../lib/i18n'

export const Route = createFileRoute('/_admin/logs')({
  component: SystemLogsPage,
})

type TimeRangeOption = 'all' | '15m' | '1h' | '24h'

function SystemLogsPage() {
  const { t } = useI18n()

  // Query Parameters State
  const [page, setPage] = useState(1)
  const [perPage, setPerPage] = useState(30)
  const [selectedType, setSelectedType] = useState<'all' | 'server' | 'request'>('all')
  const [selectedLevel, setSelectedLevel] = useState<string>('ALL')
  const [keyword, setKeyword] = useState('')
  const [debouncedKeyword, setDebouncedKeyword] = useState('')
  const [statusCodeFilter, setStatusCodeFilter] = useState<string>('all')
  const [timeRange, setTimeRange] = useState<TimeRangeOption>('all')
  const [targetFilter, setTargetFilter] = useState('')
  const [debouncedTarget, setDebouncedTarget] = useState('')

  // UI state
  const [autoScroll, setAutoScroll] = useState(true)
  const [autoRefresh, setAutoRefresh] = useState(true)
  const [selectedLog, setSelectedLog] = useState<LogEntry | null>(null)
  const [copiedNotification, setCopiedNotification] = useState(false)

  const logEndRef = useRef<HTMLDivElement>(null)

  // Debounce search and target input
  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedKeyword(keyword)
      setDebouncedTarget(targetFilter)
      setPage(1)
    }, 300)
    return () => clearTimeout(timer)
  }, [keyword, targetFilter])

  // Calculate from date based on timeRange
  const fromTime = useMemo(() => {
    if (timeRange === '15m') {
      return new Date(Date.now() - 15 * 60 * 1000).toISOString()
    }
    if (timeRange === '1h') {
      return new Date(Date.now() - 60 * 60 * 1000).toISOString()
    }
    if (timeRange === '24h') {
      return new Date(Date.now() - 24 * 60 * 60 * 1000).toISOString()
    }
    return undefined
  }, [timeRange])

  // Build query parameters
  const queryParams: LogQueryParams = useMemo(() => {
    const params: LogQueryParams = {
      page,
      perPage,
    }
    if (selectedType !== 'all') {
      params.logType = selectedType
    }
    if (selectedLevel !== 'ALL') {
      params.level = selectedLevel.toLowerCase()
    }
    if (debouncedKeyword.trim()) {
      params.q = debouncedKeyword.trim()
    }
    if (debouncedTarget.trim()) {
      params.target = debouncedTarget.trim()
    }
    if (statusCodeFilter !== 'all') {
      const parsed = parseInt(statusCodeFilter, 10)
      if (!isNaN(parsed)) {
        params.statusCode = parsed
      }
    }
    if (fromTime) {
      params.from = fromTime
    }
    return params
  }, [page, perPage, selectedType, selectedLevel, debouncedKeyword, debouncedTarget, statusCodeFilter, fromTime])

  // Fetch logs using TanStack Query
  const { data: logsResponse, isLoading, isFetching, refetch } = useQuery({
    queryKey: ['admin-logs', queryParams],
    queryFn: async () => {
      const res = await hbApi.logs.list(queryParams)
      return res.data
    },
    refetchInterval: autoRefresh ? 4000 : false,
  })

  const logs = useMemo(() => logsResponse?.data || [], [logsResponse])
  const meta = logsResponse?.meta as { total?: number; page?: number; perPage?: number } | undefined
  const total = meta?.total ?? logs.length
  const totalPages = Math.max(1, Math.ceil(total / perPage))

  useEffect(() => {
    if (autoScroll && logs.length > 0) {
      logEndRef.current?.scrollIntoView({ behavior: 'smooth' })
    }
  }, [logs, autoScroll])

  const handleCopyLogs = () => {
    if (logs.length === 0) return
    const text = logs
      .map((l) => {
        const prefix = `[${l.created_at}] [${l.level.toUpperCase()}] [${l.log_type.toUpperCase()}] [${l.target}]`
        const reqInfo = l.method && l.path ? ` ${l.method} ${l.path} (${l.status_code || '-'})` : ''
        return `${prefix}${reqInfo} ${l.message}`
      })
      .join('\n')
    navigator.clipboard.writeText(text)
    setCopiedNotification(true)
    setTimeout(() => setCopiedNotification(false), 2000)
  }

  const handleExportJson = () => {
    if (logs.length === 0) return
    const jsonStr = JSON.stringify(logs, null, 2)
    const blob = new Blob([jsonStr], { type: 'application/json' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `herta-logs-${new Date().toISOString().slice(0, 19).replace(/:/g, '-')}.json`
    a.click()
    URL.revokeObjectURL(url)
  }

  const getLevelColor = (lvl: string) => {
    const upper = lvl.toUpperCase()
    switch (upper) {
      case 'TRACE':
        return '#787b80'
      case 'DEBUG':
        return '#9876aa'
      case 'INFO':
        return '#3574f0'
      case 'WARN':
        return '#e5934e'
      case 'ERROR':
      case 'FATAL':
        return '#e53935'
      default:
        return 'var(--jb-text-muted)'
    }
  }

  const getStatusColor = (code?: number) => {
    if (!code) return 'var(--jb-text-muted)'
    if (code >= 200 && code < 300) return '#499c54' // Green
    if (code >= 300 && code < 400) return '#3574f0' // Blue
    if (code >= 400 && code < 500) return '#e5934e' // Orange
    return '#e53935' // Red (5xx)
  }

  const getMethodBadgeColor = (method?: string) => {
    switch (method?.toUpperCase()) {
      case 'GET':
        return '#3574f0'
      case 'POST':
        return '#499c54'
      case 'PUT':
      case 'PATCH':
        return '#e5934e'
      case 'DELETE':
        return '#e53935'
      default:
        return 'var(--jb-text-muted)'
    }
  }

  return (
    <div style={{ flex: 1, display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden' }}>
      {/* 1. Editor Tab Header */}
      <div className="jb-editor-tabs" style={{ justifyContent: 'space-between', paddingRight: 10 }}>
        <div style={{ display: 'flex', alignItems: 'center' }}>
          <div className="jb-editor-tab active">
            <span className="i-ph:terminal-window-bold text-amber-500 text-13px" />
            <span>{t('logs.title')}</span>
            <span
              style={{
                fontSize: 10,
                color: 'var(--jb-text-muted)',
                background: 'var(--jb-border)',
                padding: '1px 6px',
                borderRadius: 10,
                fontWeight: 600,
              }}
            >
              {total}
            </span>
          </div>

          {/* Log Type Switcher Tabs */}
          <div style={{ display: 'flex', gap: 2, marginLeft: 16 }}>
            {(['all', 'server', 'request'] as const).map((typeKey) => {
              const active = selectedType === typeKey
              return (
                <button
                  key={typeKey}
                  onClick={() => {
                    setSelectedType(typeKey)
                    setPage(1)
                  }}
                  style={{
                    background: active ? 'var(--jb-active-item)' : 'transparent',
                    border: 'none',
                    color: active ? 'var(--jb-accent-blue)' : 'var(--jb-text-muted)',
                    fontSize: 11,
                    fontWeight: active ? 700 : 500,
                    padding: '3px 10px',
                    borderRadius: 4,
                    cursor: 'pointer',
                    display: 'flex',
                    alignItems: 'center',
                    gap: 4,
                    transition: 'all 0.15s',
                  }}
                >
                  {typeKey === 'all' && <span className="i-ph:stack-bold text-12px" />}
                  {typeKey === 'server' && <span className="i-ph:cpu-bold text-12px text-purple-400" />}
                  {typeKey === 'request' && <span className="i-ph:globe-bold text-12px text-sky-400" />}
                  <span>
                    {typeKey === 'all'
                      ? t('logs.type.all')
                      : typeKey === 'server'
                        ? t('logs.type.server')
                        : t('logs.type.request')}
                  </span>
                </button>
              )
            })}
          </div>
        </div>

        {/* Action Toolbar */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
          {copiedNotification && (
            <span style={{ fontSize: 11, color: 'var(--jb-accent-green)', fontWeight: 600, marginRight: 4 }}>
              ✓ {t('app.copied')}
            </span>
          )}

          <Tooltip title={autoRefresh ? 'Live Polling Active' : 'Live Polling Paused'}>
            <button
              onClick={() => setAutoRefresh(!autoRefresh)}
              style={{
                background: autoRefresh ? 'rgba(73, 156, 84, 0.15)' : 'transparent',
                border: '1px solid var(--jb-border)',
                color: autoRefresh ? 'var(--jb-accent-green)' : 'var(--jb-text-muted)',
                borderRadius: 4,
                padding: '2px 8px',
                height: 24,
                fontSize: 11,
                cursor: 'pointer',
                display: 'flex',
                alignItems: 'center',
                gap: 4,
                fontWeight: 600,
              }}
            >
              <span
                className={`i-ph:broadcast-bold text-12px ${autoRefresh ? 'text-emerald-500 animate-pulse' : ''}`}
              />
              <span>{t('logs.auto_refresh')}</span>
            </button>
          </Tooltip>

          <Button
            onClick={() => refetch()}
            style={{ height: 24, fontSize: 11, display: 'flex', alignItems: 'center', gap: 4 }}
          >
            <span className={`i-ph:arrow-clockwise-bold text-11px ${isFetching ? 'animate-spin' : ''}`} />
            <span>{t('app.refresh')}</span>
          </Button>

          <Button
            onClick={handleCopyLogs}
            style={{ height: 24, fontSize: 11, display: 'flex', alignItems: 'center', gap: 4 }}
          >
            <span className="i-ph:copy-bold text-11px" />
            <span>{t('app.copy')}</span>
          </Button>

          <Button
            onClick={handleExportJson}
            style={{ height: 24, fontSize: 11, display: 'flex', alignItems: 'center', gap: 4 }}
          >
            <span className="i-ph:download-simple-bold text-11px" />
            <span>{t('app.export')}</span>
          </Button>
        </div>
      </div>

      {/* 2. Breadcrumbs */}
      <div className="jb-breadcrumbs">
        <span>HertaBase</span>
        <span className="jb-breadcrumb-sep">›</span>
        <span>Monitoring</span>
        <span className="jb-breadcrumb-sep">›</span>
        <span style={{ color: 'var(--jb-accent-blue)', fontWeight: 500 }}>System & Request Logs</span>
      </div>

      {/* 3. Filters Toolbar */}
      <div
        style={{
          padding: '6px 12px',
          background: 'var(--jb-header-bg)',
          borderBottom: '1px solid var(--jb-border)',
          display: 'flex',
          gap: 8,
          alignItems: 'center',
          flexWrap: 'wrap',
        }}
      >
        {/* Severity Level Pills */}
        <div style={{ display: 'flex', gap: 3 }}>
          {['ALL', 'ERROR', 'WARN', 'INFO', 'DEBUG', 'TRACE'].map((lvl) => {
            const isActive = selectedLevel === lvl
            const color = lvl === 'ALL' ? 'var(--jb-text)' : getLevelColor(lvl)
            return (
              <button
                key={lvl}
                onClick={() => {
                  setSelectedLevel(lvl)
                  setPage(1)
                }}
                style={{
                  background: isActive ? `${color}22` : 'var(--jb-panel-bg)',
                  border: isActive ? `1px solid ${color}` : '1px solid var(--jb-border)',
                  color: isActive ? color : 'var(--jb-text-muted)',
                  fontWeight: isActive ? 700 : 500,
                  borderRadius: 4,
                  padding: '2px 8px',
                  fontSize: 11,
                  cursor: 'pointer',
                  display: 'flex',
                  alignItems: 'center',
                  gap: 4,
                  transition: 'all 0.12s',
                }}
              >
                {lvl === 'ERROR' && <span className="i-ph:x-circle-bold text-rose-500" />}
                {lvl === 'WARN' && <span className="i-ph:warning-bold text-amber-500" />}
                {lvl === 'INFO' && <span className="i-ph:info-bold text-blue-500" />}
                {lvl === 'DEBUG' && <span className="i-ph:bug-bold text-purple-400" />}
                {lvl === 'TRACE' && <span className="i-ph:magnifying-glass-bold text-zinc-400" />}
                <span>{lvl === 'ALL' ? t('logs.level.all') : lvl}</span>
              </button>
            )
          })}
        </div>

        <div style={{ width: 1, height: 18, background: 'var(--jb-border)', margin: '0 2px' }} />

        {/* Search Keyword */}
        <div style={{ flex: 1, minWidth: 200, display: 'flex', alignItems: 'center', position: 'relative' }}>
          <span className="i-ph:magnifying-glass-bold text-12px text-zinc-400 absolute left-2.5" />
          <input
            type="text"
            placeholder={t('logs.search_placeholder')}
            value={keyword}
            onChange={(e) => setKeyword(e.target.value)}
            style={{
              width: '100%',
              padding: '3px 26px 3px 24px',
              borderRadius: 4,
              border: '1px solid var(--jb-border)',
              backgroundColor: 'var(--jb-editor-bg)',
              color: 'var(--jb-text)',
              fontSize: 11,
              boxSizing: 'border-box',
              height: 24,
            }}
          />
          {keyword && (
            <button
              onClick={() => setKeyword('')}
              style={{
                position: 'absolute',
                right: 6,
                background: 'transparent',
                border: 'none',
                color: 'var(--jb-text-muted)',
                cursor: 'pointer',
                display: 'flex',
                padding: 0,
              }}
            >
              <span className="i-ph:x-circle-fill text-12px" />
            </button>
          )}
        </div>

        {/* Target Module Filter */}
        <div style={{ width: 140, display: 'flex', alignItems: 'center', position: 'relative' }}>
          <span className="i-ph:tree-structure-bold text-11px text-zinc-400 absolute left-2" />
          <input
            type="text"
            placeholder={t('logs.target')}
            value={targetFilter}
            onChange={(e) => setTargetFilter(e.target.value)}
            style={{
              width: '100%',
              padding: '3px 20px 3px 20px',
              borderRadius: 4,
              border: '1px solid var(--jb-border)',
              backgroundColor: 'var(--jb-editor-bg)',
              color: 'var(--jb-text)',
              fontSize: 11,
              boxSizing: 'border-box',
              height: 24,
            }}
          />
          {targetFilter && (
            <button
              onClick={() => setTargetFilter('')}
              style={{
                position: 'absolute',
                right: 4,
                background: 'transparent',
                border: 'none',
                color: 'var(--jb-text-muted)',
                cursor: 'pointer',
                display: 'flex',
                padding: 0,
              }}
            >
              <span className="i-ph:x-circle-fill text-11px" />
            </button>
          )}
        </div>

        {/* Time Range Filter */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
          <span className="i-ph:clock-bold text-12px text-zinc-400" />
          <select
            value={timeRange}
            onChange={(e) => {
              setTimeRange(e.target.value as TimeRangeOption)
              setPage(1)
            }}
            style={{
              background: 'var(--jb-panel-bg)',
              border: '1px solid var(--jb-border)',
              color: 'var(--jb-text)',
              fontSize: 11,
              borderRadius: 4,
              padding: '2px 6px',
              height: 24,
              cursor: 'pointer',
            }}
          >
            <option value="all">{t('logs.time_all')}</option>
            <option value="15m">{t('logs.time_15m')}</option>
            <option value="1h">{t('logs.time_1h')}</option>
            <option value="24h">{t('logs.time_24h')}</option>
          </select>
        </div>

        {/* HTTP Status Code Filter (for Request Logs) */}
        {selectedType !== 'server' && (
          <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
            <span className="i-ph:traffic-signal-bold text-12px text-zinc-400" />
            <select
              value={statusCodeFilter}
              onChange={(e) => {
                setStatusCodeFilter(e.target.value)
                setPage(1)
              }}
              style={{
                background: 'var(--jb-panel-bg)',
                border: '1px solid var(--jb-border)',
                color: 'var(--jb-text)',
                fontSize: 11,
                borderRadius: 4,
                padding: '2px 6px',
                height: 24,
                cursor: 'pointer',
              }}
            >
              <option value="all">HTTP: All</option>
              <option value="200">200 OK</option>
              <option value="201">201 Created</option>
              <option value="400">400 Bad Request</option>
              <option value="401">401 Unauthorized</option>
              <option value="403">403 Forbidden</option>
              <option value="404">404 Not Found</option>
              <option value="500">500 Server Error</option>
            </select>
          </div>
        )}

        {/* Auto Scroll Toggle */}
        <label
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 4,
            fontSize: 11,
            cursor: 'pointer',
            color: 'var(--jb-text-muted)',
            marginLeft: 4,
          }}
        >
          <input
            type="checkbox"
            checked={autoScroll}
            onChange={(e) => setAutoScroll(e.target.checked)}
          />
          <span>{t('logs.auto_scroll')}</span>
        </label>
      </div>

      {/* 4. Terminal Log Stream View */}
      <div
        style={{
          flex: 1,
          backgroundColor: 'var(--jb-dock-bg)',
          color: 'var(--jb-console-text)',
          fontFamily: 'JetBrains Mono, Menlo, Consolas, monospace',
          fontSize: 11.5,
          padding: '8px 12px',
          overflowY: 'auto',
          lineHeight: 1.55,
          position: 'relative',
        }}
      >
        {isLoading && logs.length === 0 ? (
          <div style={{ display: 'flex', justifyContent: 'center', alignItems: 'center', height: '100%', gap: 8 }}>
            <Loader />
            <span style={{ color: 'var(--jb-text-muted)', fontSize: 12 }}>{t('app.loading')}</span>
          </div>
        ) : logs.length === 0 ? (
          <div style={{ padding: 48, textAlign: 'center', color: 'var(--jb-text-muted)' }}>
            <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 8 }}>
              <span className="i-ph:scroll-bold text-36px text-zinc-500" />
              <span style={{ fontSize: 13, fontWeight: 500 }}>{t('logs.empty')}</span>
              <span style={{ fontSize: 11 }}>Try adjusting your search terms or filters</span>
            </div>
          </div>
        ) : (
          logs.map((log) => {
            const lvlColor = getLevelColor(log.level)
            const isRequest = log.log_type === 'request'
            const methodColor = getMethodBadgeColor(log.method)
            const statusColor = getStatusColor(log.status_code)
            const isSelected = selectedLog?.id === log.id

            return (
              <div
                key={log.id}
                onClick={() => setSelectedLog(log)}
                style={{
                  display: 'flex',
                  gap: 8,
                  padding: '3px 6px',
                  borderRadius: 3,
                  cursor: 'pointer',
                  transition: 'background-color 0.1s',
                  alignItems: 'baseline',
                  borderLeft: isSelected ? `2px solid var(--jb-accent-blue)` : '2px solid transparent',
                  background: isSelected ? 'var(--jb-active-item)' : 'transparent',
                }}
                className="hover:bg-[var(--jb-hover-item)]"
              >
                {/* Timestamp */}
                <span
                  style={{
                    color: 'var(--jb-text-muted)',
                    fontSize: 11,
                    flexShrink: 0,
                    minWidth: 76,
                  }}
                >
                  {new Date(log.created_at).toLocaleTimeString()}
                </span>

                {/* Level Badge */}
                <span
                  style={{
                    color: lvlColor,
                    fontWeight: 700,
                    fontSize: 10.5,
                    width: 48,
                    textAlign: 'center',
                    background: `${lvlColor}18`,
                    borderRadius: 3,
                    padding: '0 3px',
                    flexShrink: 0,
                  }}
                >
                  {log.level.toUpperCase()}
                </span>

                {/* Type Badge */}
                <span
                  style={{
                    fontSize: 10,
                    color: isRequest ? '#3574f0' : '#9876aa',
                    background: isRequest ? 'rgba(53, 116, 240, 0.12)' : 'rgba(152, 118, 170, 0.12)',
                    padding: '0 4px',
                    borderRadius: 2,
                    fontWeight: 600,
                    flexShrink: 0,
                  }}
                >
                  {isRequest ? 'HTTP' : 'SYS'}
                </span>

                {/* HTTP Method & Status Code (If Request Log) */}
                {isRequest && log.method && (
                  <span
                    style={{
                      color: methodColor,
                      fontWeight: 700,
                      fontSize: 10.5,
                      flexShrink: 0,
                    }}
                  >
                    {log.method}
                  </span>
                )}

                {isRequest && log.status_code && (
                  <span
                    style={{
                      color: statusColor,
                      fontWeight: 700,
                      fontSize: 10.5,
                      background: `${statusColor}18`,
                      padding: '0 4px',
                      borderRadius: 2,
                      flexShrink: 0,
                    }}
                  >
                    {log.status_code}
                  </span>
                )}

                {/* Target Module */}
                <span
                  style={{
                    color: 'var(--jb-accent-purple)',
                    fontSize: 11,
                    maxWidth: 160,
                    overflow: 'hidden',
                    textOverflow: 'ellipsis',
                    whiteSpace: 'nowrap',
                    flexShrink: 0,
                  }}
                  title={log.target}
                >
                  {log.target}
                </span>

                {/* Message Content */}
                <span style={{ flex: 1, color: 'var(--jb-text)', wordBreak: 'break-all' }}>
                  {log.message}
                </span>

                {/* Remote IP & Caller Hint */}
                {log.remote_ip && (
                  <span
                    style={{
                      color: 'var(--jb-text-muted)',
                      fontSize: 10,
                      flexShrink: 0,
                      opacity: 0.8,
                    }}
                  >
                    {log.remote_ip}
                  </span>
                )}
              </div>
            )
          })
        )}
        <div ref={logEndRef} />
      </div>

      {/* 5. Pagination & Status Footer */}
      <div
        style={{
          height: 32,
          background: 'var(--jb-header-bg)',
          borderTop: '1px solid var(--jb-border)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '0 12px',
          fontSize: 11,
          color: 'var(--jb-text-muted)',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <span>
            {t('logs.total_records', { total })}
          </span>
          <span>•</span>
          <span>
            {t('logs.page', { page })} / {totalPages}
          </span>
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          {/* Per Page Selector */}
          <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
            <span>{t('logs.per_page', { perPage: '' })}</span>
            <select
              value={perPage}
              onChange={(e) => {
                setPerPage(parseInt(e.target.value, 10))
                setPage(1)
              }}
              style={{
                background: 'var(--jb-panel-bg)',
                border: '1px solid var(--jb-border)',
                color: 'var(--jb-text)',
                fontSize: 11,
                borderRadius: 4,
                padding: '1px 4px',
                cursor: 'pointer',
              }}
            >
              <option value="30">30</option>
              <option value="50">50</option>
              <option value="100">100</option>
              <option value="200">200</option>
            </select>
          </div>

          {/* Prev / Next Page Buttons */}
          <div style={{ display: 'flex', gap: 2 }}>
            <Button
              disabled={page <= 1}
              onClick={() => setPage((p) => Math.max(1, p - 1))}
              style={{ height: 22, padding: '0 6px', fontSize: 11 }}
            >
              <span className="i-ph:caret-left-bold" />
            </Button>
            <Button
              disabled={page >= totalPages}
              onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
              style={{ height: 22, padding: '0 6px', fontSize: 11 }}
            >
              <span className="i-ph:caret-right-bold" />
            </Button>
          </div>
        </div>
      </div>

      {/* 6. JetBrains Log Detail Inspector Drawer / Modal */}
      {selectedLog && (
        <div
          style={{
            position: 'fixed',
            inset: 0,
            backgroundColor: 'rgba(0, 0, 0, 0.6)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            zIndex: 1000,
            backdropFilter: 'blur(2px)',
          }}
          onClick={() => setSelectedLog(null)}
        >
          <div
            onClick={(e) => e.stopPropagation()}
            style={{
              width: 640,
              maxHeight: '85vh',
              background: 'var(--jb-panel-bg)',
              border: '1px solid var(--jb-border)',
              borderRadius: 8,
              boxShadow: 'var(--jb-shadow)',
              display: 'flex',
              flexDirection: 'column',
              overflow: 'hidden',
            }}
          >
            {/* Inspector Header */}
            <div
              style={{
                padding: '8px 14px',
                background: 'var(--jb-header-bg)',
                borderBottom: '1px solid var(--jb-border)',
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
              }}
            >
              <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <span className="i-ph:terminal-window-bold text-sky-400 text-15px" />
                <span style={{ fontSize: 12.5, fontWeight: 700, color: 'var(--jb-text-heading)' }}>
                  {t('logs.inspector')}
                </span>
                <span
                  style={{
                    color: getLevelColor(selectedLog.level),
                    fontSize: 10.5,
                    fontWeight: 700,
                    background: `${getLevelColor(selectedLog.level)}18`,
                    padding: '1px 6px',
                    borderRadius: 3,
                  }}
                >
                  {selectedLog.level.toUpperCase()}
                </span>
                <span
                  style={{
                    fontSize: 10.5,
                    color: selectedLog.log_type === 'request' ? '#3574f0' : '#9876aa',
                    background: selectedLog.log_type === 'request' ? 'rgba(53, 116, 240, 0.12)' : 'rgba(152, 118, 170, 0.12)',
                    padding: '1px 6px',
                    borderRadius: 3,
                    fontWeight: 600,
                  }}
                >
                  {selectedLog.log_type === 'request' ? 'HTTP Request' : 'Server Internal'}
                </span>
              </div>

              <button
                onClick={() => setSelectedLog(null)}
                style={{
                  background: 'transparent',
                  border: 'none',
                  color: 'var(--jb-text-muted)',
                  cursor: 'pointer',
                  display: 'flex',
                  padding: 2,
                }}
              >
                <span className="i-ph:x-bold text-14px" />
              </button>
            </div>

            {/* Inspector Content */}
            <div
              style={{
                padding: 14,
                display: 'flex',
                flexDirection: 'column',
                gap: 12,
                fontSize: 11.5,
                overflowY: 'auto',
              }}
            >
              {/* Key metadata grid */}
              <div
                style={{
                  display: 'grid',
                  gridTemplateColumns: 'repeat(2, 1fr)',
                  gap: 8,
                  background: 'var(--jb-dock-bg)',
                  padding: 10,
                  borderRadius: 6,
                  border: '1px solid var(--jb-border)',
                }}
              >
                <div>
                  <span style={{ color: 'var(--jb-text-muted)' }}>Timestamp: </span>
                  <span style={{ fontFamily: 'JetBrains Mono, monospace', fontWeight: 600 }}>
                    {selectedLog.created_at}
                  </span>
                </div>

                <div>
                  <span style={{ color: 'var(--jb-text-muted)' }}>Target: </span>
                  <span style={{ fontFamily: 'JetBrains Mono, monospace', color: 'var(--jb-accent-purple)', fontWeight: 600 }}>
                    {selectedLog.target}
                  </span>
                </div>

                {selectedLog.method && (
                  <div>
                    <span style={{ color: 'var(--jb-text-muted)' }}>HTTP Method: </span>
                    <span style={{ fontWeight: 700, color: getMethodBadgeColor(selectedLog.method) }}>
                      {selectedLog.method}
                    </span>
                  </div>
                )}

                {selectedLog.status_code && (
                  <div>
                    <span style={{ color: 'var(--jb-text-muted)' }}>Status Code: </span>
                    <span style={{ fontWeight: 700, color: getStatusColor(selectedLog.status_code) }}>
                      {selectedLog.status_code}
                    </span>
                  </div>
                )}

                {selectedLog.path && (
                  <div style={{ gridColumn: 'span 2' }}>
                    <span style={{ color: 'var(--jb-text-muted)' }}>Request Path: </span>
                    <span style={{ fontFamily: 'JetBrains Mono, monospace', fontWeight: 600 }}>
                      {selectedLog.path}
                    </span>
                  </div>
                )}

                {selectedLog.remote_ip && (
                  <div>
                    <span style={{ color: 'var(--jb-text-muted)' }}>Remote IP: </span>
                    <span style={{ fontFamily: 'JetBrains Mono, monospace' }}>{selectedLog.remote_ip}</span>
                  </div>
                )}

                {selectedLog.auth_type && (
                  <div>
                    <span style={{ color: 'var(--jb-text-muted)' }}>Auth Type: </span>
                    <span style={{ fontWeight: 600 }}>{selectedLog.auth_type}</span>
                  </div>
                )}

                {selectedLog.user_id && (
                  <div style={{ gridColumn: 'span 2' }}>
                    <span style={{ color: 'var(--jb-text-muted)' }}>User / Identity: </span>
                    <span style={{ fontFamily: 'JetBrains Mono, monospace' }}>
                      {selectedLog.user_id} ({selectedLog.user_collection || '_users'})
                    </span>
                  </div>
                )}

                {selectedLog.referer && (
                  <div style={{ gridColumn: 'span 2' }}>
                    <span style={{ color: 'var(--jb-text-muted)' }}>Referer: </span>
                    <span style={{ wordBreak: 'break-all' }}>{selectedLog.referer}</span>
                  </div>
                )}

                {selectedLog.user_agent && (
                  <div style={{ gridColumn: 'span 2' }}>
                    <span style={{ color: 'var(--jb-text-muted)' }}>User Agent: </span>
                    <span style={{ fontSize: 10.5, color: 'var(--jb-text)', wordBreak: 'break-all' }}>
                      {selectedLog.user_agent}
                    </span>
                  </div>
                )}
              </div>

              {/* Message Payload */}
              <div>
                <div style={{ fontWeight: 600, marginBottom: 4, color: 'var(--jb-text-heading)' }}>
                  Message Content:
                </div>
                <pre
                  style={{
                    margin: 0,
                    fontFamily: 'JetBrains Mono, monospace',
                    background: 'var(--jb-dock-bg)',
                    padding: 10,
                    borderRadius: 6,
                    border: '1px solid var(--jb-border)',
                    wordBreak: 'break-all',
                    whiteSpace: 'pre-wrap',
                    lineHeight: 1.5,
                    fontSize: 11,
                    maxHeight: 200,
                    overflowY: 'auto',
                  }}
                >
                  {selectedLog.message}
                </pre>
              </div>

              {/* Raw JSON View */}
              <div>
                <div style={{ fontWeight: 600, marginBottom: 4, color: 'var(--jb-text-heading)' }}>
                  Full Raw JSON Entry:
                </div>
                <pre
                  style={{
                    margin: 0,
                    fontFamily: 'JetBrains Mono, monospace',
                    background: 'var(--jb-dock-bg)',
                    padding: 10,
                    borderRadius: 6,
                    border: '1px solid var(--jb-border)',
                    wordBreak: 'break-all',
                    whiteSpace: 'pre-wrap',
                    lineHeight: 1.4,
                    fontSize: 10.5,
                    maxHeight: 180,
                    overflowY: 'auto',
                    color: 'var(--jb-text-muted)',
                  }}
                >
                  {JSON.stringify(selectedLog, null, 2)}
                </pre>
              </div>
            </div>

            {/* Inspector Footer Actions */}
            <div
              style={{
                padding: '8px 14px',
                background: 'var(--jb-header-bg)',
                borderTop: '1px solid var(--jb-border)',
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
              }}
            >
              <Button
                onClick={() => {
                  navigator.clipboard.writeText(JSON.stringify(selectedLog, null, 2))
                  alert(t('app.copied'))
                }}
                style={{ height: 26, fontSize: 11, display: 'flex', alignItems: 'center', gap: 4 }}
              >
                <span className="i-ph:copy-bold text-11px" />
                <span>{t('app.copy')} JSON</span>
              </Button>

              <Button onClick={() => setSelectedLog(null)} style={{ height: 26, fontSize: 11 }}>
                {t('app.close')}
              </Button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
