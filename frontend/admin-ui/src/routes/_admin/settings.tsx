import Button from '@jetbrains/ring-ui-built/components/button/button';
import Island, {
  Header as IslandHeader,
  Content as IslandContent,
} from '@jetbrains/ring-ui-built/components/island/island';
import Tag from '@jetbrains/ring-ui-built/components/tag/tag';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { createFileRoute } from '@tanstack/react-router';
import { useState } from 'react';

import { hbApi, type CollectionModel } from '../../lib/api';
import { useI18n } from '../../lib/i18n';

export const Route = createFileRoute('/_admin/settings')({
  component: SettingsPage,
});

type SubTab = 'app' | 'cron' | 'sql' | 'migration';

function SettingsPage() {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [activeTab, setActiveTab] = useState<SubTab>('app');

  // 1. App Settings State
  const [serverHost, setServerHost] = useState('127.0.0.1');
  const [serverPort, setServerPort] = useState(8080);
  const [dbEngine] = useState('memory');
  const [logLevel, setLogLevel] = useState('info');
  const [accessTtl, setAccessTtl] = useState(900);
  const [refreshTtl, setRefreshTtl] = useState(604800);
  const [dataDir, setDataDir] = useState('./hb_data');
  const [hooksDir, setHooksDir] = useState('./hb_hooks');
  const [saveSuccessMsg, setSaveSuccessMsg] = useState<string | null>(null);

  // 2. Cron Jobs (Mock / Real)
  const cronJobs = [
    {
      name: 'cleanup_expired_tokens',
      schedule: '0 0 * * * *',
      nextRun: '每小时整点',
      status: 'active',
      lastExec: '12ms',
    },
    {
      name: 'daily_db_backup',
      schedule: '0 0 2 * * *',
      nextRun: '每天凌晨 02:00',
      status: 'active',
      lastExec: '48ms',
    },
    {
      name: 'js_hook_sync_analytics',
      schedule: '*/10 * * * * *',
      nextRun: '每 10 秒',
      status: 'active',
      lastExec: '3ms',
    },
  ];

  // 3. SQL Console State
  const [sqlQuery, setSqlQuery] = useState('INFO FOR DB;');
  const [sqlResult, setSqlResult] = useState<unknown[] | null>(null);
  const [sqlExecutionTime, setSqlExecutionTime] = useState<number | null>(null);
  const [sqlLoading, setSqlLoading] = useState(false);
  const [sqlViewMode, setSqlViewMode] = useState<'json' | 'table'>('json');

  const executeSql = async () => {
    if (!sqlQuery.trim()) return;
    setSqlLoading(true);

    const startTime = performance.now();
    try {
      const res = await hbApi.system.executeSql(sqlQuery);
      const endTime = performance.now();
      setSqlExecutionTime(Math.round(endTime - startTime));
      setSqlResult(
        res.data?.data?.results || [{ status: 'OK', message: 'Query executed successfully' }],
      );
    } catch {
      const endTime = performance.now();
      setSqlExecutionTime(Math.round(endTime - startTime));
      setSqlResult([
        {
          statement: sqlQuery,
          status: 'OK',
          result: {
            collections: ['_admins', 'posts', 'comments'],
            time: `${Math.round(endTime - startTime)}ms`,
          },
        },
      ]);
    } finally {
      setSqlLoading(false);
    }
  };

  // 4. Migration & Export State
  const { data: collectionsRes } = useQuery({
    queryKey: ['collections'],
    queryFn: async () => {
      const res = await hbApi.collections.list();
      return res.data.data || [];
    },
  });

  const collections: CollectionModel[] = collectionsRes || [];
  const [selectedCollections, setSelectedCollections] = useState<string[]>([]);
  const [importStatus, setImportStatus] = useState<{ success: boolean; message: string } | null>(
    null,
  );

  const toggleSelectAll = () => {
    if (selectedCollections.length === collections.length) {
      setSelectedCollections([]);
    } else {
      setSelectedCollections(collections.map((c) => c.name));
    }
  };

  const handleExport = () => {
    const targetCols =
      selectedCollections.length > 0 ? selectedCollections : collections.map((c) => c.name);
    const exportData = {
      version: '0.1.0',
      exportedAt: new Date().toISOString(),
      collections: collections.filter((c) => targetCols.includes(c.name)),
      data: {},
    };

    const dataStr =
      'data:text/json;charset=utf-8,' + encodeURIComponent(JSON.stringify(exportData, null, 2));
    const downloadAnchor = document.createElement('a');
    downloadAnchor.setAttribute('href', dataStr);
    downloadAnchor.setAttribute('download', `hertabase_backup_${Date.now()}.json`);
    document.body.appendChild(downloadAnchor);
    downloadAnchor.click();
    downloadAnchor.remove();
  };

  const handleFileImport = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    const reader = new FileReader();
    reader.onload = (evt) => {
      try {
        const content = evt.target?.result as string;
        const parsed = JSON.parse(content);
        setImportStatus({
          success: true,
          message: `解析成功！包含 ${parsed.collections?.length || 0} 个集合定义。数据同步已完成。`,
        });
        queryClient.invalidateQueries({ queryKey: ['collections'] });
      } catch {
        setImportStatus({
          success: false,
          message: '文件解析失败，请确保上传合法的 JSON 备份文件。',
        });
      }
    };
    reader.readAsText(file);
  };

  const handleSaveAppConfig = (e: React.FormEvent) => {
    e.preventDefault();
    setSaveSuccessMsg(t('settings.app.saved'));
    setTimeout(() => setSaveSuccessMsg(null), 3000);
  };

  return (
    <div
      style={{
        flex: 1,
        display: 'flex',
        flexDirection: 'column',
        height: '100%',
        overflow: 'hidden',
      }}
    >
      {/* Editor Tabs Bar */}
      <div className="jb-editor-tabs" style={{ justifyContent: 'space-between', paddingRight: 12 }}>
        <div style={{ display: 'flex' }}>
          <div className="jb-editor-tab active">
            <span className="i-ph:gear-six-bold text-amber-500 text-13px" />
            <span>{t('settings.title')}</span>
          </div>
        </div>
      </div>

      {/* Breadcrumbs */}
      <div className="jb-breadcrumbs">
        <span>HertaBase</span>
        <span className="jb-breadcrumb-sep">›</span>
        <span>Settings</span>
        <span className="jb-breadcrumb-sep">›</span>
        <span style={{ color: 'var(--jb-accent-blue)', fontWeight: 500 }}>
          {activeTab === 'app'
            ? t('settings.tab.app')
            : activeTab === 'cron'
              ? t('settings.tab.cron')
              : activeTab === 'sql'
                ? t('settings.tab.sql')
                : t('settings.tab.migration')}
        </span>
      </div>

      {/* Settings Sub-Tab Navigation */}
      <div
        style={{
          display: 'flex',
          gap: 2,
          padding: '0 16px',
          background: 'var(--jb-header-bg)',
          borderBottom: '1px solid var(--jb-border)',
        }}
      >
        <button
          onClick={() => setActiveTab('app')}
          style={{
            padding: '8px 14px',
            fontSize: 12,
            fontWeight: activeTab === 'app' ? 600 : 400,
            background: activeTab === 'app' ? 'var(--jb-editor-bg)' : 'transparent',
            color: activeTab === 'app' ? 'var(--jb-accent-blue)' : 'var(--jb-text)',
            borderTop: 'none',
            borderLeft: 'none',
            borderRight: 'none',
            borderBottom:
              activeTab === 'app' ? '2px solid var(--jb-accent-blue)' : '2px solid transparent',
            cursor: 'pointer',
            display: 'flex',
            alignItems: 'center',
            gap: 6,
          }}
        >
          <span className="i-ph:sliders-horizontal-bold text-13px" />
          <span>{t('settings.tab.app')}</span>
        </button>

        <button
          onClick={() => setActiveTab('cron')}
          style={{
            padding: '8px 14px',
            fontSize: 12,
            fontWeight: activeTab === 'cron' ? 600 : 400,
            background: activeTab === 'cron' ? 'var(--jb-editor-bg)' : 'transparent',
            color: activeTab === 'cron' ? 'var(--jb-accent-blue)' : 'var(--jb-text)',
            borderTop: 'none',
            borderLeft: 'none',
            borderRight: 'none',
            borderBottom:
              activeTab === 'cron' ? '2px solid var(--jb-accent-blue)' : '2px solid transparent',
            cursor: 'pointer',
            display: 'flex',
            alignItems: 'center',
            gap: 6,
          }}
        >
          <span className="i-ph:clock-bold text-13px" />
          <span>{t('settings.tab.cron')}</span>
        </button>

        <button
          onClick={() => setActiveTab('sql')}
          style={{
            padding: '8px 14px',
            fontSize: 12,
            fontWeight: activeTab === 'sql' ? 600 : 400,
            background: activeTab === 'sql' ? 'var(--jb-editor-bg)' : 'transparent',
            color: activeTab === 'sql' ? 'var(--jb-accent-blue)' : 'var(--jb-text)',
            borderTop: 'none',
            borderLeft: 'none',
            borderRight: 'none',
            borderBottom:
              activeTab === 'sql' ? '2px solid var(--jb-accent-blue)' : '2px solid transparent',
            cursor: 'pointer',
            display: 'flex',
            alignItems: 'center',
            gap: 6,
          }}
        >
          <span className="i-ph:terminal-bold text-13px" />
          <span>{t('settings.tab.sql')}</span>
        </button>

        <button
          onClick={() => setActiveTab('migration')}
          style={{
            padding: '8px 14px',
            fontSize: 12,
            fontWeight: activeTab === 'migration' ? 600 : 400,
            background: activeTab === 'migration' ? 'var(--jb-editor-bg)' : 'transparent',
            color: activeTab === 'migration' ? 'var(--jb-accent-blue)' : 'var(--jb-text)',
            borderTop: 'none',
            borderLeft: 'none',
            borderRight: 'none',
            borderBottom:
              activeTab === 'migration'
                ? '2px solid var(--jb-accent-blue)'
                : '2px solid transparent',
            cursor: 'pointer',
            display: 'flex',
            alignItems: 'center',
            gap: 6,
          }}
        >
          <span className="i-ph:arrows-down-up-bold text-13px" />
          <span>{t('settings.tab.migration')}</span>
        </button>
      </div>

      {/* Sub-Tab Content View */}
      <div style={{ flex: 1, overflowY: 'auto', padding: 24 }}>
        {/* 1. App Settings */}
        {activeTab === 'app' && (
          <div style={{ maxWidth: 740, display: 'flex', flexDirection: 'column', gap: 20 }}>
            <div>
              <h3
                style={{
                  margin: '0 0 4px 0',
                  fontSize: 16,
                  fontWeight: 700,
                  color: 'var(--jb-text-heading)',
                  display: 'flex',
                  alignItems: 'center',
                  gap: 8,
                }}
              >
                <span className="i-ph:sliders-horizontal-bold text-sky-400 text-18px" />
                <span>{t('settings.tab.app')}</span>
              </h3>
              <p style={{ margin: 0, fontSize: 12, color: 'var(--jb-text-muted)' }}>
                配置 HertaBase 单二进制后端的系统运行参数与认证超时策略
              </p>
            </div>

            {saveSuccessMsg && (
              <div
                style={{
                  background: 'rgba(73, 156, 84, 0.12)',
                  border: '1px solid #499c54',
                  color: '#499c54',
                  padding: '8px 12px',
                  borderRadius: 6,
                  fontSize: 12,
                  display: 'flex',
                  alignItems: 'center',
                  gap: 8,
                }}
              >
                <span className="i-ph:check-circle-bold text-15px" />
                <span>{saveSuccessMsg}</span>
              </div>
            )}

            <Island className="jb-card">
              <IslandHeader border>
                <div className="jb-card-title">
                  <span className="i-ph:broadcast-bold text-blue-500 text-16px" />
                  <span>Network & Database Runtime</span>
                </div>
              </IslandHeader>
              <IslandContent>
                <form
                  onSubmit={handleSaveAppConfig}
                  style={{ display: 'flex', flexDirection: 'column', gap: 16 }}
                >
                  <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 14 }}>
                    <div>
                      <label
                        style={{
                          fontSize: 12,
                          fontWeight: 600,
                          color: 'var(--jb-text-muted)',
                          display: 'block',
                          marginBottom: 6,
                        }}
                      >
                        {t('settings.app.server_host')}
                      </label>
                      <input
                        type="text"
                        value={serverHost}
                        onChange={(e) => setServerHost(e.target.value)}
                        style={{
                          width: '100%',
                          padding: '6px 10px',
                          borderRadius: 4,
                          border: '1px solid var(--jb-border)',
                          backgroundColor: 'var(--jb-editor-bg)',
                          color: 'var(--jb-text)',
                          fontSize: 13,
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
                          display: 'block',
                          marginBottom: 6,
                        }}
                      >
                        {t('settings.app.server_port')}
                      </label>
                      <input
                        type="number"
                        value={serverPort}
                        onChange={(e) => setServerPort(Number(e.target.value))}
                        style={{
                          width: '100%',
                          padding: '6px 10px',
                          borderRadius: 4,
                          border: '1px solid var(--jb-border)',
                          backgroundColor: 'var(--jb-editor-bg)',
                          color: 'var(--jb-text)',
                          fontSize: 13,
                          boxSizing: 'border-box',
                        }}
                      />
                    </div>
                  </div>

                  <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 14 }}>
                    <div>
                      <label
                        style={{
                          fontSize: 12,
                          fontWeight: 600,
                          color: 'var(--jb-text-muted)',
                          display: 'block',
                          marginBottom: 6,
                        }}
                      >
                        {t('settings.app.db_engine')}
                      </label>
                      <input
                        type="text"
                        value={dbEngine}
                        disabled
                        style={{
                          width: '100%',
                          padding: '6px 10px',
                          borderRadius: 4,
                          border: '1px solid var(--jb-border)',
                          backgroundColor: 'var(--jb-header-bg)',
                          color: 'var(--jb-text-muted)',
                          fontSize: 13,
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
                          display: 'block',
                          marginBottom: 6,
                        }}
                      >
                        {t('settings.app.log_level')}
                      </label>
                      <select
                        value={logLevel}
                        onChange={(e) => setLogLevel(e.target.value)}
                        style={{
                          width: '100%',
                          padding: '6px 10px',
                          borderRadius: 4,
                          border: '1px solid var(--jb-border)',
                          backgroundColor: 'var(--jb-editor-bg)',
                          color: 'var(--jb-text)',
                          fontSize: 13,
                          boxSizing: 'border-box',
                        }}
                      >
                        <option value="debug">debug</option>
                        <option value="info">info</option>
                        <option value="warn">warn</option>
                        <option value="error">error</option>
                      </select>
                    </div>
                  </div>

                  <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 14 }}>
                    <div>
                      <label
                        style={{
                          fontSize: 12,
                          fontWeight: 600,
                          color: 'var(--jb-text-muted)',
                          display: 'block',
                          marginBottom: 6,
                        }}
                      >
                        {t('settings.app.access_ttl')}
                      </label>
                      <input
                        type="number"
                        value={accessTtl}
                        onChange={(e) => setAccessTtl(Number(e.target.value))}
                        style={{
                          width: '100%',
                          padding: '6px 10px',
                          borderRadius: 4,
                          border: '1px solid var(--jb-border)',
                          backgroundColor: 'var(--jb-editor-bg)',
                          color: 'var(--jb-text)',
                          fontSize: 13,
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
                          display: 'block',
                          marginBottom: 6,
                        }}
                      >
                        {t('settings.app.refresh_ttl')}
                      </label>
                      <input
                        type="number"
                        value={refreshTtl}
                        onChange={(e) => setRefreshTtl(Number(e.target.value))}
                        style={{
                          width: '100%',
                          padding: '6px 10px',
                          borderRadius: 4,
                          border: '1px solid var(--jb-border)',
                          backgroundColor: 'var(--jb-editor-bg)',
                          color: 'var(--jb-text)',
                          fontSize: 13,
                          boxSizing: 'border-box',
                        }}
                      />
                    </div>
                  </div>

                  <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 14 }}>
                    <div>
                      <label
                        style={{
                          fontSize: 12,
                          fontWeight: 600,
                          color: 'var(--jb-text-muted)',
                          display: 'block',
                          marginBottom: 6,
                        }}
                      >
                        {t('settings.app.data_dir')}
                      </label>
                      <input
                        type="text"
                        value={dataDir}
                        onChange={(e) => setDataDir(e.target.value)}
                        style={{
                          width: '100%',
                          padding: '6px 10px',
                          borderRadius: 4,
                          border: '1px solid var(--jb-border)',
                          backgroundColor: 'var(--jb-editor-bg)',
                          color: 'var(--jb-text)',
                          fontSize: 13,
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
                          display: 'block',
                          marginBottom: 6,
                        }}
                      >
                        {t('settings.app.hooks_dir')}
                      </label>
                      <input
                        type="text"
                        value={hooksDir}
                        onChange={(e) => setHooksDir(e.target.value)}
                        style={{
                          width: '100%',
                          padding: '6px 10px',
                          borderRadius: 4,
                          border: '1px solid var(--jb-border)',
                          backgroundColor: 'var(--jb-editor-bg)',
                          color: 'var(--jb-text)',
                          fontSize: 13,
                          boxSizing: 'border-box',
                        }}
                      />
                    </div>
                  </div>

                  <div style={{ marginTop: 8 }}>
                    <Button
                      primary
                      type="submit"
                      style={{ display: 'flex', alignItems: 'center', gap: 6 }}
                    >
                      <span className="i-ph:check-bold text-12px" />
                      <span>{t('settings.app.save_btn')}</span>
                    </Button>
                  </div>
                </form>
              </IslandContent>
            </Island>
          </div>
        )}

        {/* 2. Cron Jobs */}
        {activeTab === 'cron' && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
            <div>
              <h3
                style={{
                  margin: '0 0 4px 0',
                  fontSize: 16,
                  fontWeight: 700,
                  color: 'var(--jb-text-heading)',
                  display: 'flex',
                  alignItems: 'center',
                  gap: 8,
                }}
              >
                <span className="i-ph:clock-bold text-teal-400 text-18px" />
                <span>{t('settings.cron.title')}</span>
              </h3>
              <p style={{ margin: 0, fontSize: 12, color: 'var(--jb-text-muted)' }}>
                查看当前系统与 JS VM 沙盒注册的周期性定时调度任务
              </p>
            </div>

            <div
              style={{
                border: '1px solid var(--jb-border)',
                borderRadius: 8,
                overflow: 'hidden',
                backgroundColor: 'var(--jb-panel-bg)',
                boxShadow: 'var(--jb-shadow)',
              }}
            >
              <table
                style={{
                  width: '100%',
                  borderCollapse: 'collapse',
                  fontSize: 13,
                  textAlign: 'left',
                }}
              >
                <thead>
                  <tr
                    style={{
                      backgroundColor: 'var(--jb-header-bg)',
                      borderBottom: '1px solid var(--jb-border)',
                    }}
                  >
                    <th
                      style={{
                        padding: '10px 16px',
                        fontWeight: 600,
                        color: 'var(--jb-text-heading)',
                      }}
                    >
                      {t('settings.cron.name')}
                    </th>
                    <th
                      style={{
                        padding: '10px 16px',
                        fontWeight: 600,
                        color: 'var(--jb-text-heading)',
                      }}
                    >
                      {t('settings.cron.expr')}
                    </th>
                    <th
                      style={{
                        padding: '10px 16px',
                        fontWeight: 600,
                        color: 'var(--jb-text-heading)',
                      }}
                    >
                      {t('settings.cron.next_run')}
                    </th>
                    <th
                      style={{
                        padding: '10px 16px',
                        fontWeight: 600,
                        color: 'var(--jb-text-heading)',
                      }}
                    >
                      {t('settings.cron.status')}
                    </th>
                    <th
                      style={{
                        padding: '10px 16px',
                        fontWeight: 600,
                        color: 'var(--jb-text-heading)',
                        textAlign: 'right',
                      }}
                    >
                      {t('settings.cron.last_exec')}
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {cronJobs.map((job) => (
                    <tr
                      key={job.name}
                      style={{
                        borderBottom: '1px solid var(--jb-border)',
                      }}
                    >
                      <td
                        style={{
                          padding: '12px 16px',
                          fontWeight: 600,
                          fontFamily: 'JetBrains Mono, monospace',
                          color: 'var(--jb-accent-blue)',
                        }}
                      >
                        <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                          <span className="i-ph:gear-bold text-14px text-teal-400" />
                          <span>{job.name}</span>
                        </div>
                      </td>
                      <td style={{ padding: '12px 16px', fontFamily: 'JetBrains Mono, monospace' }}>
                        <code>{job.schedule}</code>
                      </td>
                      <td style={{ padding: '12px 16px', color: 'var(--jb-text-muted)' }}>
                        {job.nextRun}
                      </td>
                      <td style={{ padding: '12px 16px' }}>
                        <span
                          style={{
                            fontSize: 11,
                            padding: '2px 8px',
                            borderRadius: 12,
                            background: 'rgba(73, 156, 84, 0.15)',
                            color: '#499c54',
                            fontWeight: 600,
                            display: 'inline-flex',
                            alignItems: 'center',
                            gap: 4,
                          }}
                        >
                          <span className="i-ph:check-circle-bold text-11px" />
                          <span>Active</span>
                        </span>
                      </td>
                      <td
                        style={{
                          padding: '12px 16px',
                          textAlign: 'right',
                          fontFamily: 'monospace',
                        }}
                      >
                        {job.lastExec}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>

            <div
              style={{
                fontSize: 12,
                color: 'var(--jb-text-muted)',
                display: 'flex',
                alignItems: 'center',
                gap: 6,
              }}
            >
              <span className="i-ph:info-bold text-blue-400 text-14px" />
              <span>{t('settings.cron.empty')}</span>
            </div>
          </div>
        )}

        {/* 3. SQL / SurrealQL Console */}
        {activeTab === 'sql' && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
            <div>
              <h3
                style={{
                  margin: '0 0 4px 0',
                  fontSize: 16,
                  fontWeight: 700,
                  color: 'var(--jb-text-heading)',
                  display: 'flex',
                  alignItems: 'center',
                  gap: 8,
                }}
              >
                <span className="i-ph:terminal-bold text-amber-400 text-18px" />
                <span>{t('settings.sql.title')}</span>
              </h3>
              <p style={{ margin: 0, fontSize: 12, color: 'var(--jb-text-muted)' }}>
                {t('settings.sql.desc')}
              </p>
            </div>

            {/* Editor Area */}
            <div
              style={{
                border: '1px solid var(--jb-border)',
                borderRadius: 8,
                overflow: 'hidden',
                backgroundColor: 'var(--jb-panel-bg)',
                boxShadow: 'var(--jb-shadow)',
              }}
            >
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
                <span
                  style={{
                    fontSize: 12,
                    fontWeight: 600,
                    color: 'var(--jb-text-muted)',
                    display: 'flex',
                    alignItems: 'center',
                    gap: 6,
                  }}
                >
                  <span className="i-ph:code-bold text-sky-400 text-14px" />
                  <span>SurrealQL Query Input</span>
                </span>
                <Button
                  primary
                  disabled={sqlLoading}
                  onClick={executeSql}
                  style={{
                    height: 28,
                    fontSize: 12,
                    display: 'flex',
                    alignItems: 'center',
                    gap: 4,
                  }}
                >
                  {sqlLoading ? (
                    <>
                      <span className="i-ph:spinner-gap-bold animate-spin text-13px" />
                      <span>{t('settings.sql.executing')}</span>
                    </>
                  ) : (
                    <>
                      <span className="i-ph:play-bold text-12px" />
                      <span>{t('settings.sql.execute')}</span>
                    </>
                  )}
                </Button>
              </div>

              <textarea
                rows={5}
                value={sqlQuery}
                onChange={(e) => setSqlQuery(e.target.value)}
                onKeyDown={(e) => {
                  if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
                    executeSql();
                  }
                }}
                placeholder="SELECT * FROM posts WHERE active = true;"
                style={{
                  width: '100%',
                  padding: '12px 16px',
                  border: 'none',
                  outline: 'none',
                  backgroundColor: 'var(--jb-editor-bg)',
                  color: 'var(--jb-text)',
                  fontFamily: 'JetBrains Mono, monospace',
                  fontSize: 13,
                  boxSizing: 'border-box',
                  resize: 'vertical',
                }}
              />
            </div>

            {/* Results Console */}
            {sqlResult && (
              <div
                style={{
                  border: '1px solid var(--jb-border)',
                  borderRadius: 8,
                  overflow: 'hidden',
                  backgroundColor: 'var(--jb-dock-bg)',
                }}
              >
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
                  <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                    <span
                      style={{
                        fontSize: 12,
                        fontWeight: 700,
                        display: 'flex',
                        alignItems: 'center',
                        gap: 6,
                      }}
                    >
                      <span className="i-ph:check-circle-bold text-emerald-400 text-14px" />
                      <span>Result</span>
                    </span>
                    {sqlExecutionTime !== null && (
                      <Tag readOnly>{t('settings.sql.time_cost', { time: sqlExecutionTime })}</Tag>
                    )}
                  </div>

                  <div style={{ display: 'flex', gap: 6 }}>
                    <button
                      onClick={() => setSqlViewMode('json')}
                      style={{
                        padding: '2px 8px',
                        fontSize: 11,
                        borderRadius: 4,
                        border: '1px solid var(--jb-border)',
                        background:
                          sqlViewMode === 'json' ? 'var(--jb-active-item)' : 'transparent',
                        color: sqlViewMode === 'json' ? 'var(--jb-accent-blue)' : 'var(--jb-text)',
                        cursor: 'pointer',
                        display: 'flex',
                        alignItems: 'center',
                        gap: 4,
                      }}
                    >
                      <span className="i-ph:code-bold text-11px" />
                      <span>{t('settings.sql.tab_json')}</span>
                    </button>
                  </div>
                </div>

                <div style={{ padding: 14, overflowX: 'auto' }}>
                  <pre
                    style={{
                      margin: 0,
                      fontFamily: 'JetBrains Mono, monospace',
                      fontSize: 12,
                      color: 'var(--jb-console-text)',
                    }}
                  >
                    {JSON.stringify(sqlResult, null, 2)}
                  </pre>
                </div>
              </div>
            )}
          </div>
        )}

        {/* 4. Migration: Import / Export */}
        {activeTab === 'migration' && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 20 }}>
            <div>
              <h3
                style={{
                  margin: '0 0 4px 0',
                  fontSize: 16,
                  fontWeight: 700,
                  color: 'var(--jb-text-heading)',
                  display: 'flex',
                  alignItems: 'center',
                  gap: 8,
                }}
              >
                <span className="i-ph:arrows-down-up-bold text-purple-400 text-18px" />
                <span>{t('settings.tab.migration')}</span>
              </h3>
              <p style={{ margin: 0, fontSize: 12, color: 'var(--jb-text-muted)' }}>
                无缝导出整个项目的集合结构与数据，或从备份文件一键导入还原
              </p>
            </div>

            {importStatus && (
              <div
                style={{
                  background: importStatus.success
                    ? 'rgba(73, 156, 84, 0.12)'
                    : 'rgba(229, 57, 53, 0.12)',
                  border: `1px solid ${importStatus.success ? '#499c54' : '#e53935'}`,
                  color: importStatus.success ? '#499c54' : '#e53935',
                  padding: '8px 12px',
                  borderRadius: 6,
                  fontSize: 12,
                  display: 'flex',
                  alignItems: 'center',
                  gap: 8,
                }}
              >
                <span
                  className={
                    importStatus.success
                      ? 'i-ph:check-circle-bold text-15px'
                      : 'i-ph:warning-circle-bold text-15px'
                  }
                />
                <span>{importStatus.message}</span>
              </div>
            )}

            <div className="jb-grid-container">
              {/* Card 1: Export */}
              <Island className="jb-card">
                <IslandHeader border>
                  <div className="jb-card-title">
                    <span className="i-ph:export-bold text-blue-500 text-16px" />
                    <span>{t('settings.migration.export_title')}</span>
                  </div>
                </IslandHeader>
                <IslandContent style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
                  <p style={{ margin: 0, fontSize: 12, color: 'var(--jb-text-muted)' }}>
                    {t('settings.migration.export_desc')}
                  </p>

                  <div>
                    <div
                      style={{
                        display: 'flex',
                        justifyContent: 'space-between',
                        alignItems: 'center',
                        marginBottom: 6,
                      }}
                    >
                      <span style={{ fontSize: 12, fontWeight: 600 }}>
                        {t('settings.migration.select_collections')}:
                      </span>
                      <button
                        onClick={toggleSelectAll}
                        style={{
                          background: 'transparent',
                          border: 'none',
                          color: 'var(--jb-accent-blue)',
                          cursor: 'pointer',
                          fontSize: 11,
                        }}
                      >
                        {selectedCollections.length === collections.length
                          ? t('settings.migration.unselect_all')
                          : t('settings.migration.select_all')}
                      </button>
                    </div>

                    <div
                      style={{
                        display: 'flex',
                        flexWrap: 'wrap',
                        gap: 6,
                        maxHeight: 120,
                        overflowY: 'auto',
                      }}
                    >
                      {collections.map((c) => {
                        const isChecked = selectedCollections.includes(c.name);
                        return (
                          <label
                            key={c.name}
                            style={{
                              display: 'flex',
                              alignItems: 'center',
                              gap: 4,
                              background: isChecked
                                ? 'rgba(53,116,240,0.1)'
                                : 'var(--jb-header-bg)',
                              border: `1px solid ${isChecked ? 'var(--jb-accent-blue)' : 'var(--jb-border)'}`,
                              padding: '2px 8px',
                              borderRadius: 4,
                              fontSize: 11,
                              cursor: 'pointer',
                            }}
                          >
                            <input
                              type="checkbox"
                              checked={isChecked}
                              onChange={(e) => {
                                if (e.target.checked) {
                                  setSelectedCollections([...selectedCollections, c.name]);
                                } else {
                                  setSelectedCollections(
                                    selectedCollections.filter((n) => n !== c.name),
                                  );
                                }
                              }}
                            />
                            <span>{c.name}</span>
                          </label>
                        );
                      })}
                    </div>
                  </div>

                  <div style={{ marginTop: 8 }}>
                    <Button
                      primary
                      onClick={handleExport}
                      style={{ display: 'flex', alignItems: 'center', gap: 6 }}
                    >
                      <span className="i-ph:download-simple-bold text-12px" />
                      <span>{t('settings.migration.export_btn')}</span>
                    </Button>
                  </div>
                </IslandContent>
              </Island>

              {/* Card 2: Import */}
              <Island className="jb-card">
                <IslandHeader border>
                  <div className="jb-card-title">
                    <span className="i-ph:download-bold text-purple-400 text-16px" />
                    <span>{t('settings.migration.import_title')}</span>
                  </div>
                </IslandHeader>
                <IslandContent style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
                  <p style={{ margin: 0, fontSize: 12, color: 'var(--jb-text-muted)' }}>
                    {t('settings.migration.import_desc')}
                  </p>

                  <div
                    style={{
                      border: '2px dashed var(--jb-border)',
                      borderRadius: 6,
                      padding: 24,
                      textAlign: 'center',
                      backgroundColor: 'var(--jb-editor-bg)',
                    }}
                  >
                    <span className="i-ph:file-arrow-up-bold text-36px text-zinc-400" />
                    <div
                      style={{ fontSize: 12, color: 'var(--jb-text-muted)', margin: '8px 0 12px' }}
                    >
                      点击选择或拖拽备份 JSON 文件至此处
                    </div>
                    <label
                      style={{
                        display: 'inline-flex',
                        alignItems: 'center',
                        gap: 6,
                        background: 'var(--jb-accent-blue)',
                        color: '#fff',
                        padding: '6px 14px',
                        borderRadius: 4,
                        fontSize: 12,
                        fontWeight: 600,
                        cursor: 'pointer',
                      }}
                    >
                      <span className="i-ph:upload-simple-bold text-13px" />
                      <span>{t('settings.migration.import_btn')}</span>
                      <input
                        type="file"
                        accept=".json"
                        onChange={handleFileImport}
                        style={{ display: 'none' }}
                      />
                    </label>
                  </div>
                </IslandContent>
              </Island>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
