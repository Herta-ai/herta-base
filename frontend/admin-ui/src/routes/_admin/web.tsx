import Button from '@jetbrains/ring-ui-built/components/button/button';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { createFileRoute } from '@tanstack/react-router';
import dayjs from 'dayjs';
import React, { useRef, useState, useMemo, type DragEvent, type ChangeEvent } from 'react';

import { AdminPageLayout } from '../../components/layout/AdminPageLayout';
import { JbCard } from '../../components/ui/JbCard';
import { JbModal } from '../../components/ui/JbModal';
import { JbStatusTag } from '../../components/ui/JbStatusTag';
import { useToast } from '../../components/ui/Toast';
import { hbApi, type WebProjectModel } from '../../lib/api';
import { useI18n } from '../../lib/i18n';

export const Route = createFileRoute('/_admin/web')({ component: WebProjectsPage });

const DEFAULT_CACHE_CONTROL = 'public, max-age=0, must-revalidate';

function formatFileSize(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
}

function formatVersionTimestamp(version: string): string {
  const parts = version.split('-');
  if (parts.length === 6) {
    const [year, month, day, hour, min, sec] = parts;
    return `${year}-${month}-${day} ${hour}:${min}:${sec}`;
  }
  return version;
}

function WebProjectsPage() {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const toast = useToast();

  // Form State
  const archiveRef = useRef<HTMLInputElement>(null);
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const [isDragOver, setIsDragOver] = useState(false);
  const [alias, setAlias] = useState('');
  const [spaFallback, setSpaFallback] = useState(true);
  const [cacheControl, setCacheControl] = useState(DEFAULT_CACHE_CONTROL);
  const [notFound, setNotFound] = useState('');

  // Modals & Panels State
  const [searchQuery, setSearchQuery] = useState('');
  const [showGuide, setShowGuide] = useState(false);
  const [editingProject, setEditingProject] = useState<WebProjectModel | null>(null);
  const [editAlias, setEditAlias] = useState('');
  const [editSpaFallback, setEditSpaFallback] = useState(true);
  const [editCacheControl, setEditCacheControl] = useState('');
  const [editNotFound, setEditNotFound] = useState('');

  // Version History Modal State
  const [versionsModalProject, setVersionsModalProject] = useState<WebProjectModel | null>(null);

  // 1. Query Web Projects
  const {
    data: projectsRes,
    isLoading,
    isFetching,
    refetch,
  } = useQuery({
    queryKey: ['webProjects'],
    queryFn: async () => {
      const res = await hbApi.webProjects.list();
      return res.data.data || [];
    },
  });

  const projects = useMemo(() => projectsRes || [], [projectsRes]);

  // 2. Query Version History for Open Modal
  const {
    data: versionsRes,
    isLoading: isLoadingVersions,
    refetch: refetchVersions,
  } = useQuery({
    queryKey: ['webProjectVersions', versionsModalProject?.name],
    queryFn: async () => {
      if (!versionsModalProject?.name) return [];
      const res = await hbApi.webProjects.versions(versionsModalProject.name);
      return res.data.data || [];
    },
    enabled: Boolean(versionsModalProject?.name),
  });

  const versionsList = versionsRes || [];

  // Deploy Mutation
  const deploy = useMutation({
    mutationFn: async () => {
      const file = selectedFile || archiveRef.current?.files?.[0];
      if (!file) {
        throw new Error('Please select a build archive file first.');
      }
      const formData = new FormData();
      formData.append('archive', file);
      if (alias.trim()) formData.append('alias', alias.trim());
      formData.append('spaFallback', String(spaFallback));
      if (cacheControl.trim()) formData.append('cacheControl', cacheControl.trim());
      if (notFound.trim()) formData.append('notFound', notFound.trim());
      return hbApi.webProjects.deploy(formData);
    },
    onSuccess: (res) => {
      queryClient.invalidateQueries({ queryKey: ['webProjects'] });
      queryClient.invalidateQueries({ queryKey: ['webProjectVersions'] });
      toast.success(t('web.deploy_success', { id: res.data?.data?.name || 'OK' }));
      setSelectedFile(null);
      if (archiveRef.current) archiveRef.current.value = '';
      setAlias('');
      setNotFound('');
      setCacheControl(DEFAULT_CACHE_CONTROL);
      setSpaFallback(true);
    },
    onError: (err: {
      response?: { data?: { error?: { message?: string } } };
      message?: string;
    }) => {
      toast.error(err.response?.data?.error?.message || err.message || 'Deploy failed');
    },
  });

  // Update Settings Mutation
  const updateSettings = useMutation({
    mutationFn: async ({
      name,
      data,
    }: {
      name: string;
      data: {
        alias?: string | null;
        spaFallback?: boolean;
        cacheControl?: string;
        notFound?: string | null;
      };
    }) => {
      return hbApi.webProjects.patch(name, data);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['webProjects'] });
      toast.success(t('web.update_success'));
      setEditingProject(null);
    },
    onError: (err: {
      response?: { data?: { error?: { message?: string } } };
      message?: string;
    }) => {
      toast.error(err.response?.data?.error?.message || err.message || 'Update failed');
    },
  });

  // Rollback Version Mutation
  const rollback = useMutation({
    mutationFn: async ({ name, version }: { name: string; version: string }) => {
      return hbApi.webProjects.rollback(name, version);
    },
    onSuccess: (_, variables) => {
      queryClient.invalidateQueries({ queryKey: ['webProjects'] });
      queryClient.invalidateQueries({ queryKey: ['webProjectVersions'] });
      toast.success(
        t('web.rollback_success', { version: formatVersionTimestamp(variables.version) }),
      );
      setVersionsModalProject(null);
    },
    onError: (err: {
      response?: { data?: { error?: { message?: string } } };
      message?: string;
    }) => {
      toast.error(err.response?.data?.error?.message || err.message || 'Rollback failed');
    },
  });

  // Delete Mutation
  const remove = useMutation({
    mutationFn: async (name: string) => {
      return hbApi.webProjects.delete(name);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['webProjects'] });
      queryClient.invalidateQueries({ queryKey: ['webProjectVersions'] });
      toast.success(t('web.deleted_success'));
    },
    onError: (err: {
      response?: { data?: { error?: { message?: string } } };
      message?: string;
    }) => {
      toast.error(err.response?.data?.error?.message || err.message || 'Delete failed');
    },
  });

  const handleOpenVersions = (project: WebProjectModel) => {
    setVersionsModalProject(project);
  };

  const handleOpenEdit = (proj: WebProjectModel) => {
    setEditingProject(proj);
    setEditAlias(proj.alias || '');
    setEditSpaFallback(proj.spaFallback);
    setEditCacheControl(proj.cacheControl || DEFAULT_CACHE_CONTROL);
    setEditNotFound(proj.notFound || '');
  };

  const handleSaveEdit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!editingProject) return;
    updateSettings.mutate({
      name: editingProject.name,
      data: {
        alias: editAlias.trim() || null,
        spaFallback: editSpaFallback,
        cacheControl: editCacheControl.trim() || DEFAULT_CACHE_CONTROL,
        notFound: editNotFound.trim() || null,
      },
    });
  };

  const handleFileChange = (e: ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) setSelectedFile(file);
  };

  const handleDragOver = (e: DragEvent<HTMLDivElement>) => {
    e.preventDefault();
    setIsDragOver(true);
  };

  const handleDragLeave = (e: DragEvent<HTMLDivElement>) => {
    e.preventDefault();
    setIsDragOver(false);
  };

  const handleDrop = (e: DragEvent<HTMLDivElement>) => {
    e.preventDefault();
    setIsDragOver(false);
    const file = e.dataTransfer.files?.[0];
    if (file) setSelectedFile(file);
  };

  const handleCopyProjectUrl = (path: string) => {
    const origin = window.location.origin;
    const url = `${origin}${path}`;
    navigator.clipboard.writeText(url);
    toast.success(t('app.copied'));
  };

  const applyCachePreset = (preset: string) => {
    setCacheControl(preset);
  };

  const filteredProjects = useMemo(() => {
    const q = searchQuery.trim().toLowerCase();
    if (!q) return projects;
    return projects.filter(
      (p) => p.name.toLowerCase().includes(q) || (p.alias && p.alias.toLowerCase().includes(q)),
    );
  }, [projects, searchQuery]);

  return (
    <AdminPageLayout
      tabTitle={t('web.title')}
      tabIcon="i-ph:globe-bold text-sky-400"
      tabBadge={projects.length}
      breadcrumbs={[{ label: t('web.projects_list'), icon: 'i-ph:globe-bold text-sky-400' }]}
      actions={
        <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
          <Button
            onClick={() => setShowGuide(!showGuide)}
            style={{
              height: 24,
              fontSize: 11,
              display: 'flex',
              alignItems: 'center',
              gap: 4,
              background: showGuide ? 'var(--jb-active-item)' : 'transparent',
              color: showGuide ? 'var(--jb-accent-blue)' : 'var(--jb-text)',
            }}
          >
            <span className="i-ph:book-bookmark-bold text-12px" />
            <span>{t('web.guide.title')}</span>
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
      banner={
        <div className="jb-welcome-banner">
          <div className="jb-banner-info">
            <h2>{t('web.title')}</h2>
            <p>{t('web.subtitle')}</p>
            <div className="jb-banner-tags">
              <span className="jb-tag-blue">
                <span className="i-ph:rocket-launch-bold" />
                <span>{t('web.projects_count', { count: projects.length })}</span>
              </span>
              <span className="jb-tag-green">
                <span className="i-ph:shield-check-bold" />
                <span>Atomic Zero-Downtime Rollback</span>
              </span>
            </div>
          </div>
        </div>
      }
    >
      {/* Optional Guide Card */}
      {showGuide && (
        <JbCard title={t('web.guide.title')} icon="i-ph:info-bold text-sky-400">
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(auto-fit, minmax(260px, 1fr))',
              gap: 12,
            }}
          >
            <div
              style={{
                background: 'var(--jb-header-bg)',
                padding: '10px 12px',
                borderRadius: 6,
                border: '1px solid var(--jb-border)',
              }}
            >
              <div style={{ fontWeight: 600, color: 'var(--jb-accent-blue)', marginBottom: 4 }}>
                1. {t('web.guide.step1')}
              </div>
              <div style={{ color: 'var(--jb-text-muted)', fontSize: 11, lineHeight: 1.5 }}>
                {t('web.guide.rule1')}
              </div>
            </div>
            <div
              style={{
                background: 'var(--jb-header-bg)',
                padding: '10px 12px',
                borderRadius: 6,
                border: '1px solid var(--jb-border)',
              }}
            >
              <div style={{ fontWeight: 600, color: 'var(--jb-accent-purple)', marginBottom: 4 }}>
                2. {t('web.guide.step2')}
              </div>
              <div style={{ color: 'var(--jb-text-muted)', fontSize: 11, lineHeight: 1.5 }}>
                {t('web.guide.rule2')}
              </div>
            </div>
            <div
              style={{
                background: 'var(--jb-header-bg)',
                padding: '10px 12px',
                borderRadius: 6,
                border: '1px solid var(--jb-border)',
              }}
            >
              <div style={{ fontWeight: 600, color: 'var(--jb-accent-green)', marginBottom: 4 }}>
                3. {t('web.guide.step3')}
              </div>
              <div style={{ color: 'var(--jb-text-muted)', fontSize: 11, lineHeight: 1.5 }}>
                {t('web.guide.rule3')}
              </div>
            </div>
          </div>
        </JbCard>
      )}

      {/* Deployment Section Card */}
      <JbCard
        title={t('web.deploy_title')}
        icon="i-ph:cloud-arrow-up-bold text-sky-400"
        subtitle={t('web.deploy_desc')}
      >
        <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
          {/* File Upload Box */}
          <div
            onDragOver={handleDragOver}
            onDragLeave={handleDragLeave}
            onDrop={handleDrop}
            onClick={() => archiveRef.current?.click()}
            style={{
              border: isDragOver
                ? '2px dashed var(--jb-accent-blue)'
                : selectedFile
                  ? '1.5px solid var(--jb-accent-green)'
                  : '1.5px dashed var(--jb-border)',
              background: isDragOver
                ? 'rgba(53, 116, 240, 0.08)'
                : selectedFile
                  ? 'rgba(73, 156, 84, 0.06)'
                  : 'var(--jb-header-bg)',
              borderRadius: 6,
              padding: selectedFile ? '10px 16px' : '16px 20px',
              cursor: 'pointer',
              display: 'flex',
              alignItems: 'center',
              justifyContent: selectedFile ? 'space-between' : 'center',
              gap: 12,
              boxSizing: 'border-box',
            }}
          >
            <input
              ref={archiveRef}
              type="file"
              accept=".zip,.7z,.tar.gz,application/zip,application/x-7z-compressed,application/gzip,application/x-tar"
              onChange={handleFileChange}
              style={{ display: 'none' }}
            />

            {selectedFile ? (
              <>
                <div style={{ display: 'flex', alignItems: 'center', gap: 10, minWidth: 0 }}>
                  <span className="i-ph:file-zip-bold text-26px text-emerald-500 shrink-0" />
                  <div style={{ minWidth: 0 }}>
                    <div
                      style={{
                        fontSize: 12,
                        fontWeight: 700,
                        color: 'var(--jb-text-heading)',
                        overflow: 'hidden',
                        textOverflow: 'ellipsis',
                        whiteSpace: 'nowrap',
                      }}
                    >
                      {selectedFile.name}
                    </div>
                    <div style={{ fontSize: 11, color: 'var(--jb-text-muted)', marginTop: 2 }}>
                      {formatFileSize(selectedFile.size)} · {selectedFile.type || 'archive'}
                    </div>
                  </div>
                </div>
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    setSelectedFile(null);
                    if (archiveRef.current) archiveRef.current.value = '';
                  }}
                  style={{
                    background: 'transparent',
                    border: '1px solid var(--jb-border)',
                    color: 'var(--jb-text-muted)',
                    borderRadius: 4,
                    padding: '3px 8px',
                    fontSize: 11,
                    cursor: 'pointer',
                    display: 'flex',
                    alignItems: 'center',
                    gap: 4,
                  }}
                >
                  <span className="i-ph:x-bold text-11px" />
                  <span>清除重选</span>
                </button>
              </>
            ) : (
              <div style={{ textAlign: 'center' }}>
                <span className="i-ph:cloud-arrow-up-bold text-28px text-sky-400 mb-1 inline-block" />
                <div style={{ fontSize: 13, fontWeight: 600, color: 'var(--jb-text-heading)' }}>
                  {t('web.drop_archive')}
                </div>
                <div style={{ fontSize: 11, color: 'var(--jb-text-muted)', marginTop: 3 }}>
                  {t('web.drop_help')}
                </div>
              </div>
            )}
          </div>

          {/* Form Options Grid */}
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))',
              gap: 12,
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
                {t('web.alias')}
              </label>
              <input
                type="text"
                value={alias}
                onChange={(e) => setAlias(e.target.value)}
                placeholder={t('web.alias_placeholder')}
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
              <div style={{ fontSize: 11, color: 'var(--jb-text-muted)', marginTop: 4 }}>
                {t('web.alias_help')}
              </div>
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
                {t('web.not_found')}
              </label>
              <input
                type="text"
                value={notFound}
                onChange={(e) => setNotFound(e.target.value)}
                placeholder={t('web.not_found_placeholder')}
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
              <div style={{ fontSize: 11, color: 'var(--jb-text-muted)', marginTop: 4 }}>
                {t('web.not_found_help')}
              </div>
            </div>

            <div style={{ gridColumn: '1 / -1' }}>
              <div
                style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'center',
                  marginBottom: 4,
                  flexWrap: 'wrap',
                  gap: 4,
                }}
              >
                <label style={{ fontSize: 12, fontWeight: 600, color: 'var(--jb-text-muted)' }}>
                  {t('web.cache_control')}
                </label>
                <div style={{ display: 'flex', gap: 4, flexWrap: 'wrap', alignItems: 'center' }}>
                  <span style={{ fontSize: 10, color: 'var(--jb-text-muted)', marginRight: 2 }}>
                    快捷预设:
                  </span>
                  <button
                    type="button"
                    onClick={() => applyCachePreset('no-cache, no-store, must-revalidate')}
                    style={{
                      background: 'var(--jb-header-bg)',
                      border: '1px solid var(--jb-border)',
                      color: 'var(--jb-text)',
                      fontSize: 10,
                      padding: '1px 6px',
                      borderRadius: 3,
                      cursor: 'pointer',
                    }}
                  >
                    {t('web.cache_preset.no_cache')}
                  </button>
                  <button
                    type="button"
                    onClick={() => applyCachePreset('public, max-age=3600, must-revalidate')}
                    style={{
                      background: 'var(--jb-header-bg)',
                      border: '1px solid var(--jb-border)',
                      color: 'var(--jb-text)',
                      fontSize: 10,
                      padding: '1px 6px',
                      borderRadius: 3,
                      cursor: 'pointer',
                    }}
                  >
                    {t('web.cache_preset.one_hour')}
                  </button>
                  <button
                    type="button"
                    onClick={() => applyCachePreset('public, max-age=86400, must-revalidate')}
                    style={{
                      background: 'var(--jb-header-bg)',
                      border: '1px solid var(--jb-border)',
                      color: 'var(--jb-text)',
                      fontSize: 10,
                      padding: '1px 6px',
                      borderRadius: 3,
                      cursor: 'pointer',
                    }}
                  >
                    {t('web.cache_preset.one_day')}
                  </button>
                  <button
                    type="button"
                    onClick={() => applyCachePreset('public, max-age=31536000, immutable')}
                    style={{
                      background: 'var(--jb-header-bg)',
                      border: '1px solid var(--jb-border)',
                      color: 'var(--jb-text)',
                      fontSize: 10,
                      padding: '1px 6px',
                      borderRadius: 3,
                      cursor: 'pointer',
                    }}
                  >
                    {t('web.cache_preset.immutable')}
                  </button>
                </div>
              </div>
              <input
                type="text"
                value={cacheControl}
                onChange={(e) => setCacheControl(e.target.value)}
                placeholder={t('web.cache_control_placeholder')}
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

          {/* SPA Fallback Toggle */}
          <div
            style={{
              background: 'var(--jb-header-bg)',
              border: '1px solid var(--jb-border)',
              borderRadius: 6,
              padding: '8px 12px',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
              <span className="i-ph:arrows-split-bold text-sky-400 text-14px" />
              <span style={{ fontSize: 12, fontWeight: 600, color: 'var(--jb-text-heading)' }}>
                {t('web.spa_fallback')}
              </span>
              <span style={{ fontSize: 11, color: 'var(--jb-text-muted)', marginLeft: 4 }}>
                ({t('web.spa_fallback_help')})
              </span>
            </div>
            <label
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 5,
                cursor: 'pointer',
                fontSize: 12,
              }}
            >
              <input
                type="checkbox"
                checked={spaFallback}
                onChange={(e) => setSpaFallback(e.target.checked)}
                style={{ cursor: 'pointer' }}
              />
              <span
                style={{
                  fontWeight: 600,
                  color: spaFallback ? 'var(--jb-accent-blue)' : 'var(--jb-text-muted)',
                }}
              >
                {spaFallback ? 'ON' : 'OFF'}
              </span>
            </label>
          </div>

          {/* Deploy Action */}
          <div>
            <Button
              primary
              onClick={() => deploy.mutate()}
              disabled={deploy.isPending || (!selectedFile && !archiveRef.current?.files?.[0])}
              style={{
                height: 28,
                fontSize: 12,
                display: 'flex',
                alignItems: 'center',
                gap: 6,
                padding: '0 14px',
              }}
            >
              {deploy.isPending ? (
                <>
                  <span className="i-ph:spinner-gap-bold animate-spin text-13px" />
                  <span>{t('web.deploying')}</span>
                </>
              ) : (
                <>
                  <span className="i-ph:rocket-launch-bold text-13px" />
                  <span>{t('web.deploy_btn')}</span>
                </>
              )}
            </Button>
          </div>
        </div>
      </JbCard>

      {/* Deployed Projects Section Card */}
      <JbCard
        noBodyPadding
        title={t('web.deployed_projects')}
        icon="i-ph:browsers-bold text-blue-400"
        actions={
          <div style={{ position: 'relative', width: 220, display: 'flex', alignItems: 'center' }}>
            <span className="i-ph:magnifying-glass-bold text-12px text-zinc-400 absolute left-2.5" />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="搜索项目或 Alias..."
              style={{
                width: '100%',
                padding: '4px 8px 4px 26px',
                borderRadius: 4,
                border: '1px solid var(--jb-border)',
                backgroundColor: 'var(--jb-editor-bg)',
                color: 'var(--jb-text)',
                fontSize: 11,
                boxSizing: 'border-box',
              }}
            />
          </div>
        }
      >
        <div style={{ overflowX: 'auto' }}>
          <table
            style={{ width: '100%', borderCollapse: 'collapse', textAlign: 'left', fontSize: 12 }}
          >
            <thead>
              <tr
                style={{
                  background: 'var(--jb-header-bg)',
                  borderBottom: '1px solid var(--jb-border)',
                }}
              >
                <th
                  style={{
                    padding: '10px 14px',
                    fontWeight: 600,
                    color: 'var(--jb-text-heading)',
                    width: 140,
                  }}
                >
                  {t('web.table.project_id')}
                </th>
                <th
                  style={{
                    padding: '10px 14px',
                    fontWeight: 600,
                    color: 'var(--jb-text-heading)',
                    width: 130,
                  }}
                >
                  {t('web.table.alias')}
                </th>
                <th
                  style={{
                    padding: '10px 14px',
                    fontWeight: 600,
                    color: 'var(--jb-text-heading)',
                    width: 100,
                  }}
                >
                  {t('web.table.spa')}
                </th>
                <th
                  style={{
                    padding: '10px 14px',
                    fontWeight: 600,
                    color: 'var(--jb-text-heading)',
                    width: 150,
                  }}
                >
                  {t('web.table.updated_at')}
                </th>
                <th
                  style={{
                    padding: '10px 14px',
                    fontWeight: 600,
                    color: 'var(--jb-text-heading)',
                    textAlign: 'right',
                    width: 220,
                  }}
                >
                  {t('app.actions')}
                </th>
              </tr>
            </thead>
            <tbody>
              {isLoading ? (
                <tr>
                  <td
                    colSpan={5}
                    style={{
                      padding: '30px 16px',
                      textAlign: 'center',
                      color: 'var(--jb-text-muted)',
                    }}
                  >
                    <div
                      style={{
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        gap: 8,
                      }}
                    >
                      <span className="i-ph:spinner-gap-bold animate-spin text-16px text-sky-400" />
                      <span>{t('app.loading')}</span>
                    </div>
                  </td>
                </tr>
              ) : filteredProjects.length === 0 ? (
                <tr>
                  <td
                    colSpan={5}
                    style={{
                      padding: '40px 16px',
                      textAlign: 'center',
                      color: 'var(--jb-text-muted)',
                    }}
                  >
                    <div
                      style={{
                        display: 'flex',
                        flexDirection: 'column',
                        alignItems: 'center',
                        gap: 6,
                      }}
                    >
                      <span className="i-ph:globe-hemisphere-west-bold text-30px opacity-40" />
                      <span>{t('web.empty')}</span>
                    </div>
                  </td>
                </tr>
              ) : (
                filteredProjects.map((p) => {
                  const projectPath = `/web/${p.alias || p.name}/`;
                  return (
                    <tr
                      key={p.name}
                      style={{
                        borderBottom: '1px solid var(--jb-border)',
                        transition: 'background-color 0.15s',
                      }}
                      className="hover:bg-[var(--jb-active-item)]"
                    >
                      <td style={{ padding: '10px 14px' }}>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                          <span className="i-ph:folder-simple-bold text-sky-400 text-14px shrink-0" />
                          <span
                            style={{
                              fontWeight: 600,
                              color: 'var(--jb-text-heading)',
                              fontFamily: 'monospace',
                            }}
                          >
                            {p.name}
                          </span>
                        </div>
                      </td>

                      <td style={{ padding: '10px 14px' }}>
                        {p.alias ? (
                          <JbStatusTag variant="info" icon="i-ph:link-simple-bold" size="sm">
                            {p.alias}
                          </JbStatusTag>
                        ) : (
                          <span style={{ color: 'var(--jb-text-muted)', fontSize: 11 }}>-</span>
                        )}
                      </td>

                      <td style={{ padding: '10px 14px' }}>
                        <JbStatusTag variant={p.spaFallback ? 'info' : 'default'} size="sm">
                          {p.spaFallback ? 'SPA: ON' : 'OFF'}
                        </JbStatusTag>
                      </td>

                      <td
                        style={{
                          padding: '10px 14px',
                          color: 'var(--jb-text-muted)',
                          fontSize: 11,
                        }}
                      >
                        {p.deployedAt ? dayjs(p.deployedAt).format('YYYY-MM-DD HH:mm') : '-'}
                      </td>

                      <td style={{ padding: '10px 14px', textAlign: 'right' }}>
                        <div style={{ display: 'flex', gap: 6, justifyContent: 'flex-end' }}>
                          <a
                            href={projectPath}
                            target="_blank"
                            rel="noreferrer"
                            title={t('web.open_browser')}
                            style={{
                              background: 'transparent',
                              border: '1px solid var(--jb-border)',
                              color: 'var(--jb-accent-blue)',
                              borderRadius: 4,
                              padding: '3px 6px',
                              fontSize: 11,
                              display: 'inline-flex',
                              alignItems: 'center',
                              gap: 3,
                              textDecoration: 'none',
                            }}
                          >
                            <span className="i-ph:arrow-square-out-bold" />
                          </a>

                          <button
                            type="button"
                            onClick={() => handleCopyProjectUrl(projectPath)}
                            title={t('web.copy_url')}
                            style={{
                              background: 'transparent',
                              border: '1px solid var(--jb-border)',
                              color: 'var(--jb-text)',
                              borderRadius: 4,
                              padding: '3px 6px',
                              fontSize: 11,
                              cursor: 'pointer',
                              display: 'inline-flex',
                              alignItems: 'center',
                            }}
                          >
                            <span className="i-ph:copy-bold" />
                          </button>

                          <button
                            type="button"
                            onClick={() => handleOpenVersions(p)}
                            title={t('web.view_versions')}
                            style={{
                              background: 'transparent',
                              border: '1px solid var(--jb-border)',
                              color: 'var(--jb-accent-purple)',
                              borderRadius: 4,
                              padding: '3px 6px',
                              fontSize: 11,
                              cursor: 'pointer',
                              display: 'inline-flex',
                              alignItems: 'center',
                              gap: 3,
                            }}
                          >
                            <span className="i-ph:clock-counter-clockwise-bold" />
                          </button>

                          <button
                            type="button"
                            onClick={() => handleOpenEdit(p)}
                            title={t('app.edit')}
                            style={{
                              background: 'transparent',
                              border: '1px solid var(--jb-border)',
                              color: 'var(--jb-text)',
                              borderRadius: 4,
                              padding: '3px 6px',
                              fontSize: 11,
                              cursor: 'pointer',
                              display: 'inline-flex',
                              alignItems: 'center',
                            }}
                          >
                            <span className="i-ph:pencil-simple-bold" />
                          </button>

                          <button
                            type="button"
                            onClick={() => {
                              if (window.confirm(t('web.delete_confirm', { id: p.name }))) {
                                remove.mutate(p.name);
                              }
                            }}
                            title={t('app.delete')}
                            style={{
                              background: 'transparent',
                              border: '1px solid var(--jb-border)',
                              color: '#ef4444',
                              borderRadius: 4,
                              padding: '3px 6px',
                              fontSize: 11,
                              cursor: 'pointer',
                              display: 'inline-flex',
                              alignItems: 'center',
                            }}
                          >
                            <span className="i-ph:trash-bold" />
                          </button>
                        </div>
                      </td>
                    </tr>
                  );
                })
              )}
            </tbody>
          </table>
        </div>
      </JbCard>

      {/* Edit Settings Modal */}
      <JbModal
        open={Boolean(editingProject)}
        onClose={() => setEditingProject(null)}
        width={560}
        title={`${t('web.edit_modal.title')} (${editingProject?.name})`}
        icon="i-ph:sliders-horizontal-bold text-sky-400"
        footer={
          <>
            <Button onClick={() => setEditingProject(null)} style={{ height: 28, fontSize: 12 }}>
              {t('app.cancel')}
            </Button>
            <Button
              primary
              onClick={handleSaveEdit}
              disabled={updateSettings.isPending}
              style={{ height: 28, fontSize: 12, display: 'flex', alignItems: 'center', gap: 4 }}
            >
              {updateSettings.isPending ? (
                <>
                  <span className="i-ph:spinner-gap-bold animate-spin text-12px" />
                  <span>{t('app.loading')}</span>
                </>
              ) : (
                <>
                  <span className="i-ph:check-bold text-12px" />
                  <span>{t('app.save')}</span>
                </>
              )}
            </Button>
          </>
        }
      >
        <form
          onSubmit={handleSaveEdit}
          style={{ display: 'flex', flexDirection: 'column', gap: 14 }}
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
              {t('web.alias')}
            </label>
            <input
              type="text"
              value={editAlias}
              onChange={(e) => setEditAlias(e.target.value)}
              placeholder="e.g. docs, app, blog"
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
              {t('web.cache_control')}
            </label>
            <input
              type="text"
              value={editCacheControl}
              onChange={(e) => setEditCacheControl(e.target.value)}
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
              {t('web.not_found')}
            </label>
            <input
              type="text"
              value={editNotFound}
              onChange={(e) => setEditNotFound(e.target.value)}
              placeholder="e.g. 404.html"
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

          <div
            style={{
              background: 'var(--jb-header-bg)',
              border: '1px solid var(--jb-border)',
              borderRadius: 6,
              padding: '8px 12px',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
            }}
          >
            <span style={{ fontSize: 12, fontWeight: 600, color: 'var(--jb-text-heading)' }}>
              {t('web.spa_fallback')}
            </span>
            <input
              type="checkbox"
              checked={editSpaFallback}
              onChange={(e) => setEditSpaFallback(e.target.checked)}
              style={{ cursor: 'pointer' }}
            />
          </div>
        </form>
      </JbModal>

      {/* Version History Modal */}
      <JbModal
        open={Boolean(versionsModalProject)}
        onClose={() => setVersionsModalProject(null)}
        width={580}
        title={`${t('web.versions_modal.title')} (${versionsModalProject?.name})`}
        icon="i-ph:clock-counter-clockwise-bold text-purple-400"
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
              onClick={() => refetchVersions()}
              style={{ height: 26, fontSize: 11, display: 'flex', alignItems: 'center', gap: 4 }}
            >
              <span
                className={`i-ph:arrow-clockwise-bold text-11px ${isLoadingVersions ? 'animate-spin' : ''}`}
              />
              <span>{t('app.refresh')}</span>
            </Button>
            <Button
              onClick={() => setVersionsModalProject(null)}
              style={{ height: 28, fontSize: 12 }}
            >
              {t('app.close')}
            </Button>
          </div>
        }
      >
        <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
          {/* Current Online Info */}
          {versionsModalProject && (
            <div
              style={{
                padding: '8px 12px',
                borderRadius: 6,
                background: 'rgba(53, 116, 240, 0.08)',
                border: '1px solid rgba(53, 116, 240, 0.25)',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
              }}
            >
              <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <span className="i-ph:broadcast-bold text-sky-400 text-15px" />
                <span style={{ fontSize: 12, fontWeight: 600, color: 'var(--jb-text-heading)' }}>
                  当前线上运行版本
                </span>
              </div>
              <span style={{ fontSize: 11, color: 'var(--jb-text-muted)' }}>
                部署时间:{' '}
                {versionsModalProject.deployedAt
                  ? dayjs(versionsModalProject.deployedAt).format('YYYY-MM-DD HH:mm:ss')
                  : '-'}
              </span>
            </div>
          )}

          {/* Backup Versions List */}
          {isLoadingVersions ? (
            <div style={{ padding: 30, textAlign: 'center', color: 'var(--jb-text-muted)' }}>
              <span className="i-ph:spinner-gap-bold animate-spin text-16px text-sky-400" />
              <div style={{ marginTop: 6 }}>{t('app.loading')}</div>
            </div>
          ) : versionsList.length === 0 ? (
            <div style={{ padding: 30, textAlign: 'center', color: 'var(--jb-text-muted)' }}>
              <span className="i-ph:clock-counter-clockwise-bold text-28px opacity-40 mb-1 inline-block" />
              <div>{t('web.versions_modal.no_history')}</div>
              <div style={{ fontSize: 11, color: 'var(--jb-text-muted)', marginTop: 4 }}>
                当再次上传新构建包覆盖部署该项目时，系统会自动将当前版本归档为历史备份。
              </div>
            </div>
          ) : (
            versionsList.map((ver) => (
              <div
                key={ver}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'space-between',
                  padding: '8px 12px',
                  borderRadius: 6,
                  border: '1px solid var(--jb-border)',
                  background: 'var(--jb-header-bg)',
                }}
              >
                <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                  <span className="i-ph:archive-box-bold text-purple-400 text-16px" />
                  <div>
                    <div style={{ fontSize: 12, fontWeight: 700, color: 'var(--jb-text-heading)' }}>
                      {formatVersionTimestamp(ver)}
                    </div>
                    <div
                      style={{
                        fontSize: 10,
                        color: 'var(--jb-text-muted)',
                        fontFamily: 'monospace',
                      }}
                    >
                      {ver}
                    </div>
                  </div>
                </div>

                <Button
                  onClick={() => {
                    if (
                      window.confirm(
                        `确定要将项目 ${versionsModalProject?.name} 回滚到历史备份版本 ${ver} 吗？`,
                      )
                    ) {
                      rollback.mutate({ name: versionsModalProject!.name, version: ver });
                    }
                  }}
                  disabled={rollback.isPending}
                  style={{
                    height: 24,
                    fontSize: 11,
                    display: 'flex',
                    alignItems: 'center',
                    gap: 4,
                  }}
                >
                  <span className="i-ph:arrow-counter-clockwise-bold text-11px" />
                  <span>{t('web.versions_modal.rollback_btn')}</span>
                </Button>
              </div>
            ))
          )}
        </div>
      </JbModal>
    </AdminPageLayout>
  );
}
