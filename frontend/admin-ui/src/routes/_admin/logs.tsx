import Button from '@jetbrains/ring-ui-built/components/button/button';
import { useQuery } from '@tanstack/react-query';
import { createFileRoute } from '@tanstack/react-router';
import { useState, useRef, useEffect, useMemo } from 'react';

import { AdminPageLayout } from '../../components/layout/AdminPageLayout';
import { JbCard } from '../../components/ui/JbCard';
import { JbModal } from '../../components/ui/JbModal';
import { JbStatusTag } from '../../components/ui/JbStatusTag';
import { useToast } from '../../components/ui/Toast';
import { hbApi, type LogEntry, type LogQueryParams } from '../../lib/api';
import { useI18n } from '../../lib/i18n';

export const Route = createFileRoute('/_admin/logs')({
  component: SystemLogsPage,
});

type TimeRangeOption = 'all' | '15m' | '1h' | '24h';
const LOG_POLL_INTERVAL_MS = 4000;

function SystemLogsPage() {
  const { t } = useI18n();
  const toast = useToast();

  // Query Parameters State
  const [page, setPage] = useState(1);
  const perPage = 30;
  const [selectedType, setSelectedType] = useState<'all' | 'server' | 'request'>('all');
  const [selectedLevel, setSelectedLevel] = useState<string>('ALL');
  const [keyword, setKeyword] = useState('');
  const [debouncedKeyword, setDebouncedKeyword] = useState('');
  const [timeRange, setTimeRange] = useState<TimeRangeOption>('all');

  // UI state
  const [autoScroll, setAutoScroll] = useState(true);
  const [pollingEnabled, setPollingEnabled] = useState(true);
  const [selectedLog, setSelectedLog] = useState<LogEntry | null>(null);

  const logEndRef = useRef<HTMLDivElement>(null);

  // Debounce search
  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedKeyword(keyword);
      setPage(1);
    }, 300);
    return () => clearTimeout(timer);
  }, [keyword]);

  const fromTime = useMemo(() => {
    if (timeRange === '15m') return new Date(Date.now() - 15 * 60 * 1000).toISOString();
    if (timeRange === '1h') return new Date(Date.now() - 60 * 60 * 1000).toISOString();
    if (timeRange === '24h') return new Date(Date.now() - 24 * 60 * 60 * 1000).toISOString();
    return undefined;
  }, [timeRange]);

  const queryParams: LogQueryParams = useMemo(() => {
    const params: LogQueryParams = { page, perPage };
    if (selectedType !== 'all') params.logType = selectedType;
    if (selectedLevel !== 'ALL') params.level = selectedLevel.toLowerCase();
    if (debouncedKeyword.trim()) params.q = debouncedKeyword.trim();
    if (fromTime) params.from = fromTime;
    return params;
  }, [page, perPage, selectedType, selectedLevel, debouncedKeyword, fromTime]);

  const {
    data: logsResponse,
    isLoading,
    isFetching,
    refetch,
  } = useQuery({
    queryKey: ['admin-logs', queryParams],
    queryFn: async () => {
      const res = await hbApi.logs.list(queryParams);
      return res.data;
    },
    refetchInterval: pollingEnabled ? LOG_POLL_INTERVAL_MS : false,
  });

  const logs = useMemo(() => logsResponse?.data || [], [logsResponse]);
  const meta = logsResponse?.meta as
    | { total?: number; page?: number; perPage?: number }
    | undefined;
  const total = meta?.total ?? logs.length;
  const totalPages = Math.max(1, Math.ceil(total / perPage));

  useEffect(() => {
    if (autoScroll && logs.length > 0) {
      logEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }
  }, [logs, autoScroll]);

  const handleCopyLogs = () => {
    if (logs.length === 0) return;
    const text = logs
      .map((l) => {
        const prefix = `[${l.created_at}] [${l.level.toUpperCase()}] [${l.log_type.toUpperCase()}] [${l.target}]`;
        const reqInfo =
          l.method && l.path ? ` ${l.method} ${l.path} (${l.status_code || '-'})` : '';
        return `${prefix}${reqInfo} ${l.message}`;
      })
      .join('\n');
    navigator.clipboard.writeText(text);
    toast.success(t('app.copied'));
  };

  const handleExportJson = () => {
    if (logs.length === 0) return;
    const jsonStr = JSON.stringify(logs, null, 2);
    const blob = new Blob([jsonStr], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `hertabase_logs_${Date.now()}.json`;
    a.click();
    URL.revokeObjectURL(a.href);
    toast.success('Logs JSON exported successfully');
  };

  const getLevelVariant = (level: string) => {
    const l = level.toUpperCase();
    if (l === 'ERROR' || l === 'FATAL') return 'error';
    if (l === 'WARN') return 'warning';
    if (l === 'INFO') return 'info';
    return 'default';
  };

  return (
    <AdminPageLayout
      tabTitle={t('logs.title')}
      tabIcon="i-ph:terminal-window-bold text-emerald-400"
      tabBadge={total}
      breadcrumbs={[{ label: t('logs.title'), icon: 'i-ph:terminal-window-bold text-emerald-400' }]}
      actions={
        <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
          <Button
            onClick={() => setPollingEnabled(!pollingEnabled)}
            style={{
              height: 24,
              fontSize: 11,
              display: 'flex',
              alignItems: 'center',
              gap: 4,
              background: pollingEnabled ? 'rgba(73, 156, 84, 0.12)' : 'transparent',
              color: pollingEnabled ? 'var(--jb-accent-green)' : 'var(--jb-text-muted)',
            }}
          >
            <span
              className={`i-ph:broadcast-bold text-11px ${pollingEnabled ? 'animate-pulse' : ''}`}
            />
            <span>{pollingEnabled ? 'Live Polling: ON' : 'Live Polling: OFF'}</span>
          </Button>

          <Button
            onClick={() => setAutoScroll(!autoScroll)}
            style={{
              height: 24,
              fontSize: 11,
              display: 'flex',
              alignItems: 'center',
              gap: 4,
              color: autoScroll ? 'var(--jb-accent-blue)' : 'var(--jb-text-muted)',
            }}
          >
            <span className="i-ph:arrow-down-bold text-11px" />
            <span>{t('logs.auto_scroll')}</span>
          </Button>

          <Button
            onClick={() => refetch()}
            style={{ height: 24, fontSize: 11, display: 'flex', alignItems: 'center', gap: 4 }}
          >
            <span
              className={`i-ph:arrow-clockwise-bold text-11px ${isFetching ? 'animate-spin' : ''}`}
            />
            <span>{t('app.refresh')}</span>
          </Button>
        </div>
      }
    >
      {/* Search & Filter Toolbar */}
      <JbCard noBodyPadding>
        <div
          style={{
            padding: '10px 14px',
            backgroundColor: 'var(--jb-header-bg)',
            borderBottom: '1px solid var(--jb-border)',
            display: 'flex',
            alignItems: 'center',
            gap: 10,
            flexWrap: 'wrap',
          }}
        >
          {/* Keyword Search */}
          <div
            style={{
              position: 'relative',
              flex: 1,
              minWidth: 200,
              display: 'flex',
              alignItems: 'center',
            }}
          >
            <span className="i-ph:magnifying-glass-bold text-12px text-zinc-400 absolute left-2.5" />
            <input
              type="text"
              value={keyword}
              onChange={(e) => setKeyword(e.target.value)}
              placeholder={t('logs.search_placeholder')}
              style={{
                width: '100%',
                padding: '5px 8px 5px 26px',
                borderRadius: 4,
                border: '1px solid var(--jb-border)',
                backgroundColor: 'var(--jb-editor-bg)',
                color: 'var(--jb-text)',
                fontSize: 12,
                boxSizing: 'border-box',
              }}
            />
          </div>

          {/* Level Filter */}
          <select
            value={selectedLevel}
            onChange={(e) => setSelectedLevel(e.target.value)}
            style={{
              padding: '5px 8px',
              borderRadius: 4,
              border: '1px solid var(--jb-border)',
              backgroundColor: 'var(--jb-editor-bg)',
              color: 'var(--jb-text)',
              fontSize: 12,
            }}
          >
            <option value="ALL">All Levels</option>
            <option value="INFO">INFO</option>
            <option value="WARN">WARN</option>
            <option value="ERROR">ERROR</option>
            <option value="DEBUG">DEBUG</option>
          </select>

          {/* Type Filter */}
          <select
            value={selectedType}
            onChange={(e) => setSelectedType(e.target.value as 'all' | 'server' | 'request')}
            style={{
              padding: '5px 8px',
              borderRadius: 4,
              border: '1px solid var(--jb-border)',
              backgroundColor: 'var(--jb-editor-bg)',
              color: 'var(--jb-text)',
              fontSize: 12,
            }}
          >
            <option value="all">All Types</option>
            <option value="request">HTTP Requests</option>
            <option value="server">Server Core</option>
          </select>

          {/* Time Range Filter */}
          <select
            value={timeRange}
            onChange={(e) => setTimeRange(e.target.value as TimeRangeOption)}
            style={{
              padding: '5px 8px',
              borderRadius: 4,
              border: '1px solid var(--jb-border)',
              backgroundColor: 'var(--jb-editor-bg)',
              color: 'var(--jb-text)',
              fontSize: 12,
            }}
          >
            <option value="all">All Time</option>
            <option value="15m">Last 15m</option>
            <option value="1h">Last 1h</option>
            <option value="24h">Last 24h</option>
          </select>

          {/* Actions */}
          <div style={{ display: 'flex', gap: 6, marginLeft: 'auto' }}>
            <Button onClick={handleCopyLogs} style={{ height: 26, fontSize: 11 }}>
              <span className="i-ph:copy-bold text-11px mr-1" />
              {t('app.copy')}
            </Button>
            <Button onClick={handleExportJson} style={{ height: 26, fontSize: 11 }}>
              <span className="i-ph:download-simple-bold text-11px mr-1" />
              JSON
            </Button>
          </div>
        </div>

        {/* Logs Terminal Stream Box */}
        <div
          style={{
            maxHeight: 520,
            overflowY: 'auto',
            backgroundColor: 'var(--jb-editor-bg)',
            padding: '10px 14px',
            fontFamily: 'monospace',
            fontSize: 12,
            lineHeight: 1.6,
          }}
        >
          {isLoading ? (
            <div style={{ padding: 40, textAlign: 'center', color: 'var(--jb-text-muted)' }}>
              <span className="i-ph:spinner-gap-bold animate-spin text-20px text-sky-400" />
              <div style={{ marginTop: 8 }}>{t('app.loading')}</div>
            </div>
          ) : logs.length === 0 ? (
            <div style={{ padding: 40, textAlign: 'center', color: 'var(--jb-text-muted)' }}>
              <span className="i-ph:terminal-bold text-28px opacity-40" />
              <div style={{ marginTop: 8 }}>{t('logs.empty')}</div>
            </div>
          ) : (
            logs.map((log) => {
              const lvlVariant = getLevelVariant(log.level);
              return (
                <div
                  key={log.id}
                  onClick={() => setSelectedLog(log)}
                  style={{
                    padding: '4px 8px',
                    borderRadius: 4,
                    display: 'flex',
                    alignItems: 'baseline',
                    gap: 8,
                    cursor: 'pointer',
                    transition: 'background-color 0.1s',
                  }}
                  className="hover:bg-[var(--jb-active-item)]"
                >
                  <span style={{ color: 'var(--jb-text-muted)', fontSize: 11, flexShrink: 0 }}>
                    {log.created_at ? new Date(log.created_at).toLocaleTimeString() : '-'}
                  </span>

                  <JbStatusTag variant={lvlVariant} size="sm">
                    {log.level.toUpperCase()}
                  </JbStatusTag>

                  <span style={{ color: 'var(--jb-accent-purple)', fontSize: 11 }}>
                    [{log.target || log.log_type}]
                  </span>

                  {log.method && log.path && (
                    <span style={{ color: 'var(--jb-accent-blue)', fontWeight: 600 }}>
                      {log.method} {log.path} {log.status_code ? `(${log.status_code})` : ''}
                    </span>
                  )}

                  <span style={{ color: 'var(--jb-text)', wordBreak: 'break-word', flex: 1 }}>
                    {log.message}
                  </span>
                </div>
              );
            })
          )}
          <div ref={logEndRef} />
        </div>

        {/* Footer */}
        <div
          style={{
            padding: '8px 16px',
            background: 'var(--jb-header-bg)',
            borderTop: '1px solid var(--jb-border)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            fontSize: 12,
          }}
        >
          <span style={{ color: 'var(--jb-text-muted)' }}>
            Showing {logs.length} of {total} logs
          </span>

          <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
            <button
              type="button"
              disabled={page <= 1}
              onClick={() => setPage((p) => Math.max(1, p - 1))}
              style={{
                background: 'var(--jb-panel-bg)',
                border: '1px solid var(--jb-border)',
                color: 'var(--jb-text)',
                borderRadius: 4,
                padding: '3px 8px',
                cursor: page <= 1 ? 'not-allowed' : 'pointer',
                display: 'flex',
                alignItems: 'center',
              }}
            >
              <span className="i-ph:caret-left-bold text-12px" />
            </button>
            <span>
              {page} / {totalPages}
            </span>
            <button
              type="button"
              disabled={page >= totalPages}
              onClick={() => setPage((p) => p + 1)}
              style={{
                background: 'var(--jb-panel-bg)',
                border: '1px solid var(--jb-border)',
                color: 'var(--jb-text)',
                borderRadius: 4,
                padding: '3px 8px',
                cursor: page >= totalPages ? 'not-allowed' : 'pointer',
                display: 'flex',
                alignItems: 'center',
              }}
            >
              <span className="i-ph:caret-right-bold text-12px" />
            </button>
          </div>
        </div>
      </JbCard>

      {/* Log Detail Modal */}
      <JbModal
        open={Boolean(selectedLog)}
        onClose={() => setSelectedLog(null)}
        width={620}
        title={`Log Event #${selectedLog?.id}`}
        icon="i-ph:file-text-bold text-sky-400"
        footer={
          <div
            style={{
              display: 'flex',
              justifyContent: 'space-between',
              width: '100%',
              alignItems: 'center',
            }}
          >
            <Button
              onClick={() => {
                navigator.clipboard.writeText(JSON.stringify(selectedLog, null, 2));
                toast.success(t('app.copied'));
              }}
              style={{ height: 28, fontSize: 12, display: 'flex', alignItems: 'center', gap: 4 }}
            >
              <span className="i-ph:copy-bold text-12px" />
              <span>{t('app.copy')} JSON</span>
            </Button>
            <Button onClick={() => setSelectedLog(null)} style={{ height: 28, fontSize: 12 }}>
              {t('app.close')}
            </Button>
          </div>
        }
      >
        <pre
          style={{
            margin: 0,
            padding: 12,
            borderRadius: 6,
            background: 'var(--jb-editor-bg)',
            border: '1px solid var(--jb-border)',
            fontSize: 12,
            fontFamily: 'monospace',
            overflowX: 'auto',
            maxHeight: 420,
            color: 'var(--jb-text)',
          }}
        >
          {JSON.stringify(selectedLog, null, 2)}
        </pre>
      </JbModal>
    </AdminPageLayout>
  );
}
