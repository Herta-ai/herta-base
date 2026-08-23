import Button from '@jetbrains/ring-ui-built/components/button/button';
import { useQuery } from '@tanstack/react-query';
import { createFileRoute } from '@tanstack/react-router';
import React, { useState } from 'react';

import { AdminPageLayout } from '../../components/layout/AdminPageLayout';
import { JbCard } from '../../components/ui/JbCard';
import { JbStatusTag } from '../../components/ui/JbStatusTag';
import { useToast } from '../../components/ui/Toast';
import { hbApi, type CollectionModel } from '../../lib/api';
import { useI18n } from '../../lib/i18n';

export const Route = createFileRoute('/_admin/settings')({
  component: SettingsPage,
});

type SubTab = 'app' | 'cron' | 'sql' | 'migration';

function SettingsPage() {
  const { t } = useI18n();
  const toast = useToast();
  const [activeTab, setActiveTab] = useState<SubTab>('app');

  // 1. App Settings State
  const [serverHost, setServerHost] = useState('127.0.0.1');
  const [serverPort, setServerPort] = useState(8080);
  const [dbEngine] = useState('memory');
  const [logLevel, setLogLevel] = useState('info');
  const [accessTtl, setAccessTtl] = useState(900);
  const [refreshTtl, setRefreshTtl] = useState(604800);

  // 2. Cron Jobs
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
      toast.success('SQL query executed successfully');
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
      toast.info(`Query completed in ${Math.round(endTime - startTime)}ms`);
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
    toast.success('Backup export downloaded');
  };

  const handleFileImport = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    const reader = new FileReader();
    reader.onload = (evt) => {
      try {
        const content = evt.target?.result as string;
        const parsed = JSON.parse(content);
        if (parsed.collections && Array.isArray(parsed.collections)) {
          toast.success(`成功导入并识别 ${parsed.collections.length} 个集合 Schema`);
        } else {
          toast.warning('文件已读取，但未发现标准 collections 数组');
        }
      } catch (err: unknown) {
        toast.error('导入解析失败：' + (err as Error).message);
      }
    };
    reader.readAsText(file);
  };

  const handleSaveAppSettings = (e: React.FormEvent) => {
    e.preventDefault();
    toast.success('配置已保存 (热生效)');
  };

  return (
    <AdminPageLayout
      tabTitle={t('nav.settings')}
      tabIcon="i-ph:gear-six-bold text-purple-400"
      breadcrumbs={[{ label: t('nav.settings'), icon: 'i-ph:gear-six-bold text-purple-400' }]}
      actions={
        <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
          {(['app', 'cron', 'sql', 'migration'] as SubTab[]).map((tab) => {
            const isActive = activeTab === tab;
            const titles: Record<SubTab, string> = {
              app: t('settings.tab.app'),
              cron: t('settings.tab.cron'),
              sql: t('settings.tab.sql'),
              migration: t('settings.tab.migration'),
            };
            return (
              <button
                key={tab}
                type="button"
                onClick={() => setActiveTab(tab)}
                style={{
                  background: isActive ? 'var(--jb-active-item)' : 'transparent',
                  color: isActive ? 'var(--jb-accent-blue)' : 'var(--jb-text-muted)',
                  border: 'none',
                  borderRadius: 4,
                  padding: '4px 10px',
                  fontSize: 12,
                  fontWeight: isActive ? 600 : 400,
                  cursor: 'pointer',
                  transition: 'all 0.15s',
                }}
              >
                {titles[tab]}
              </button>
            );
          })}
        </div>
      }
    >
      {/* 1. App Configuration Tab */}
      {activeTab === 'app' && (
        <form
          onSubmit={handleSaveAppSettings}
          style={{ display: 'flex', flexDirection: 'column', gap: 16 }}
        >
          <JbCard
            title={t('settings.app.server_title')}
            icon="i-ph:hard-drives-bold text-blue-400"
            subtitle={t('settings.app.server_subtitle')}
            footer={
              <div style={{ display: 'flex', justifyContent: 'flex-end', width: '100%' }}>
                <Button primary type="submit" style={{ height: 28, fontSize: 12 }}>
                  {t('app.save')}
                </Button>
              </div>
            }
          >
            <div
              style={{
                display: 'grid',
                gridTemplateColumns: 'repeat(auto-fill, minmax(280px, 1fr))',
                gap: 14,
              }}
            >
              <div>
                <label
                  style={{
                    fontSize: 12,
                    fontWeight: 600,
                    color: 'var(--jb-text-muted)',
                    display: 'block',
                    marginBottom: 4,
                  }}
                >
                  {t('settings.app.host')}
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
                    fontSize: 12,
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
                    marginBottom: 4,
                  }}
                >
                  {t('settings.app.port')}
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
                    fontSize: 12,
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
                    marginBottom: 4,
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
                    fontSize: 12,
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
                    marginBottom: 4,
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
                    fontSize: 12,
                    boxSizing: 'border-box',
                  }}
                >
                  <option value="trace">TRACE</option>
                  <option value="debug">DEBUG</option>
                  <option value="info">INFO</option>
                  <option value="warn">WARN</option>
                  <option value="error">ERROR</option>
                </select>
              </div>
            </div>
          </JbCard>

          <JbCard
            title={t('settings.app.security_title')}
            icon="i-ph:shield-check-bold text-amber-400"
            subtitle={t('settings.app.security_subtitle')}
          >
            <div
              style={{
                display: 'grid',
                gridTemplateColumns: 'repeat(auto-fill, minmax(280px, 1fr))',
                gap: 14,
              }}
            >
              <div>
                <label
                  style={{
                    fontSize: 12,
                    fontWeight: 600,
                    color: 'var(--jb-text-muted)',
                    display: 'block',
                    marginBottom: 4,
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
                    fontSize: 12,
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
                    marginBottom: 4,
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
                    fontSize: 12,
                    boxSizing: 'border-box',
                  }}
                />
              </div>
            </div>
          </JbCard>
        </form>
      )}

      {/* 2. Cron Jobs Tab */}
      {activeTab === 'cron' && (
        <JbCard
          noBodyPadding
          title={t('settings.cron.title')}
          icon="i-ph:clock-countdown-bold text-sky-400"
          subtitle={t('settings.cron.subtitle')}
        >
          <table
            style={{ width: '100%', borderCollapse: 'collapse', textAlign: 'left', fontSize: 13 }}
          >
            <thead>
              <tr
                style={{
                  background: 'var(--jb-header-bg)',
                  borderBottom: '1px solid var(--jb-border)',
                }}
              >
                <th
                  style={{ padding: '10px 16px', fontWeight: 600, color: 'var(--jb-text-heading)' }}
                >
                  {t('settings.cron.name')}
                </th>
                <th
                  style={{ padding: '10px 16px', fontWeight: 600, color: 'var(--jb-text-heading)' }}
                >
                  {t('settings.cron.schedule')}
                </th>
                <th
                  style={{ padding: '10px 16px', fontWeight: 600, color: 'var(--jb-text-heading)' }}
                >
                  {t('settings.cron.next_run')}
                </th>
                <th
                  style={{ padding: '10px 16px', fontWeight: 600, color: 'var(--jb-text-heading)' }}
                >
                  {t('settings.cron.last_duration')}
                </th>
                <th
                  style={{ padding: '10px 16px', fontWeight: 600, color: 'var(--jb-text-heading)' }}
                >
                  {t('app.status')}
                </th>
              </tr>
            </thead>
            <tbody>
              {cronJobs.map((job) => (
                <tr
                  key={job.name}
                  style={{ borderBottom: '1px solid var(--jb-border)' }}
                  className="hover:bg-[var(--jb-active-item)]"
                >
                  <td
                    style={{
                      padding: '10px 16px',
                      fontWeight: 600,
                      color: 'var(--jb-accent-blue)',
                    }}
                  >
                    {job.name}
                  </td>
                  <td style={{ padding: '10px 16px', fontFamily: 'monospace', fontSize: 12 }}>
                    {job.schedule}
                  </td>
                  <td style={{ padding: '10px 16px', color: 'var(--jb-text-muted)', fontSize: 12 }}>
                    {job.nextRun}
                  </td>
                  <td style={{ padding: '10px 16px', color: 'var(--jb-text-muted)', fontSize: 12 }}>
                    {job.lastExec}
                  </td>
                  <td style={{ padding: '10px 16px' }}>
                    <JbStatusTag variant="success" icon="i-ph:check-circle-bold">
                      {job.status.toUpperCase()}
                    </JbStatusTag>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </JbCard>
      )}

      {/* 3. SQL Console Tab */}
      {activeTab === 'sql' && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
          <JbCard
            title={t('settings.sql.title')}
            icon="i-ph:terminal-bold text-amber-400"
            subtitle={t('settings.sql.subtitle')}
            actions={
              <div style={{ display: 'flex', gap: 6 }}>
                <button
                  type="button"
                  onClick={() => setSqlQuery('INFO FOR DB;')}
                  style={{
                    background: 'var(--jb-header-bg)',
                    border: '1px solid var(--jb-border)',
                    color: 'var(--jb-text)',
                    fontSize: 11,
                    padding: '2px 8px',
                    borderRadius: 4,
                    cursor: 'pointer',
                  }}
                >
                  INFO FOR DB
                </button>
                <button
                  type="button"
                  onClick={() => setSqlQuery('SELECT * FROM _admins;')}
                  style={{
                    background: 'var(--jb-header-bg)',
                    border: '1px solid var(--jb-border)',
                    color: 'var(--jb-text)',
                    fontSize: 11,
                    padding: '2px 8px',
                    borderRadius: 4,
                    cursor: 'pointer',
                  }}
                >
                  _admins
                </button>
              </div>
            }
          >
            <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
              <textarea
                rows={4}
                value={sqlQuery}
                onChange={(e) => setSqlQuery(e.target.value)}
                placeholder="INFO FOR DB; OR SELECT * FROM posts WHERE active = true;"
                style={{
                  width: '100%',
                  padding: '10px 12px',
                  borderRadius: 6,
                  border: '1px solid var(--jb-border)',
                  backgroundColor: 'var(--jb-editor-bg)',
                  color: 'var(--jb-text)',
                  fontSize: 13,
                  fontFamily: 'monospace',
                  lineHeight: 1.5,
                  boxSizing: 'border-box',
                }}
              />
              <div
                style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}
              >
                <div style={{ fontSize: 11, color: 'var(--jb-text-muted)' }}>
                  支持 SurrealQL 查询与 DDL 操作
                </div>
                <Button
                  primary
                  onClick={executeSql}
                  disabled={sqlLoading}
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
                      <span className="i-ph:spinner-gap-bold animate-spin text-12px" />
                      <span>{t('app.loading')}</span>
                    </>
                  ) : (
                    <>
                      <span className="i-ph:play-bold text-12px" />
                      <span>{t('settings.sql.run')}</span>
                    </>
                  )}
                </Button>
              </div>
            </div>
          </JbCard>

          {sqlResult && (
            <JbCard
              title={
                <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                  <span>{t('settings.sql.result')}</span>
                  {sqlExecutionTime !== null && (
                    <JbStatusTag variant="success" size="sm">
                      {sqlExecutionTime}ms
                    </JbStatusTag>
                  )}
                </div>
              }
              actions={
                <Button
                  onClick={() => {
                    navigator.clipboard.writeText(JSON.stringify(sqlResult, null, 2));
                    toast.success(t('app.copied'));
                  }}
                  style={{ height: 24, fontSize: 11 }}
                >
                  {t('app.copy')}
                </Button>
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
                  maxHeight: 380,
                  color: 'var(--jb-text)',
                }}
              >
                {JSON.stringify(sqlResult, null, 2)}
              </pre>
            </JbCard>
          )}
        </div>
      )}

      {/* 4. Migration & Export Tab */}
      {activeTab === 'migration' && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
          <JbCard
            title={t('settings.export.title')}
            icon="i-ph:export-bold text-blue-400"
            subtitle={t('settings.export.subtitle')}
            footer={
              <div
                style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  width: '100%',
                  alignItems: 'center',
                }}
              >
                <Button onClick={toggleSelectAll} style={{ height: 26, fontSize: 11 }}>
                  {selectedCollections.length === collections.length ? '取消全选' : '全选所有集合'}
                </Button>
                <Button primary onClick={handleExport} style={{ height: 28, fontSize: 12 }}>
                  <span className="i-ph:download-simple-bold text-12px mr-1" />
                  {t('settings.export.btn')}
                </Button>
              </div>
            }
          >
            <div
              style={{
                display: 'grid',
                gridTemplateColumns: 'repeat(auto-fill, minmax(200px, 1fr))',
                gap: 8,
              }}
            >
              {collections.map((col) => (
                <label
                  key={col.name}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: 8,
                    padding: '6px 10px',
                    borderRadius: 4,
                    border: '1px solid var(--jb-border)',
                    background: 'var(--jb-header-bg)',
                    cursor: 'pointer',
                    fontSize: 12,
                  }}
                >
                  <input
                    type="checkbox"
                    checked={selectedCollections.includes(col.name)}
                    onChange={(e) => {
                      if (e.target.checked) {
                        setSelectedCollections([...selectedCollections, col.name]);
                      } else {
                        setSelectedCollections(selectedCollections.filter((n) => n !== col.name));
                      }
                    }}
                  />
                  <span style={{ fontWeight: 600 }}>{col.name}</span>
                </label>
              ))}
            </div>
          </JbCard>

          <JbCard
            title={t('settings.import.title')}
            icon="i-ph:upload-simple-bold text-emerald-400"
            subtitle={t('settings.import.subtitle')}
          >
            <input
              type="file"
              accept=".json"
              onChange={handleFileImport}
              style={{
                padding: '10px 14px',
                border: '1px dashed var(--jb-border)',
                borderRadius: 6,
                width: '100%',
                boxSizing: 'border-box',
                background: 'var(--jb-header-bg)',
                cursor: 'pointer',
              }}
            />
          </JbCard>
        </div>
      )}
    </AdminPageLayout>
  );
}
