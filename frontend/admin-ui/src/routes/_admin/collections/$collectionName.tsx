import Button from '@jetbrains/ring-ui-built/components/button/button';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { createFileRoute } from '@tanstack/react-router';
import React, { useState } from 'react';

import { AdminPageLayout } from '../../../components/layout/AdminPageLayout';
import { JbCard } from '../../../components/ui/JbCard';
import { JbModal } from '../../../components/ui/JbModal';
import { JbStatusTag } from '../../../components/ui/JbStatusTag';
import { useToast } from '../../../components/ui/Toast';
import { hbApi, type RecordModel, type CollectionModel } from '../../../lib/api';
import { useI18n } from '../../../lib/i18n';
import { useRealtimeCollection, type RealtimeEvent } from '../../../lib/sse';

export const Route = createFileRoute('/_admin/collections/$collectionName')({
  component: CollectionRecordsPage,
});

function CollectionRecordsPage() {
  const { collectionName } = Route.useParams();
  const queryClient = useQueryClient();
  const { t } = useI18n();
  const toast = useToast();

  // Table state
  const [page, setPage] = useState(1);
  const perPage = 30;
  const [filter, setFilter] = useState('');
  const [sort, setSort] = useState('-created_at');
  const expand = '';

  // Modals state
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [editingRecord, setEditingRecord] = useState<RecordModel | null>(null);
  const [formData, setFormData] = useState<Record<string, unknown>>({});
  const [jsonViewModal, setJsonViewModal] = useState<RecordModel | null>(null);

  // 1. Fetch Collection Schema
  const { data: collectionRes } = useQuery({
    queryKey: ['collection', collectionName],
    queryFn: async () => {
      const res = await hbApi.collections.get(collectionName);
      return res.data.data;
    },
  });

  const collection: CollectionModel | undefined = collectionRes;

  // 2. Fetch Records
  const {
    data: recordsRes,
    isLoading,
    refetch,
  } = useQuery({
    queryKey: ['records', collectionName, page, perPage, filter, sort, expand],
    queryFn: async () => {
      const res = await hbApi.records.list(collectionName, {
        page,
        perPage,
        filter: filter.trim() || undefined,
        sort: sort.trim() || undefined,
        expand: expand.trim() || undefined,
      });
      return res.data;
    },
  });

  const records = recordsRes?.data || [];
  const meta = recordsRes?.meta as
    | { total?: number; totalItems?: number; page?: number; perPage?: number }
    | undefined;
  const totalCount = meta?.total ?? meta?.totalItems ?? records.length;
  const totalPages = Math.max(1, Math.ceil(totalCount / perPage));

  // 3. Realtime SSE
  const { status: sseStatus } = useRealtimeCollection(collectionName, (event: RealtimeEvent) => {
    queryClient.invalidateQueries({ queryKey: ['records', collectionName] });
    toast.info(`[Realtime] ${event.action.toUpperCase()} ${collectionName}`);
  });

  // 4. Mutations
  const createRecordMutation = useMutation({
    mutationFn: (data: Record<string, unknown>) => hbApi.records.create(collectionName, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['records', collectionName] });
      toast.success(t('records.saved_success'));
      closeModal();
    },
    onError: (err: {
      response?: { data?: { error?: { message?: string } } };
      message?: string;
    }) => {
      toast.error(err.response?.data?.error?.message || err.message || 'Create record failed');
    },
  });

  const updateRecordMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: Record<string, unknown> }) =>
      hbApi.records.update(collectionName, id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['records', collectionName] });
      toast.success(t('records.saved_success'));
      closeModal();
    },
    onError: (err: {
      response?: { data?: { error?: { message?: string } } };
      message?: string;
    }) => {
      toast.error(err.response?.data?.error?.message || err.message || 'Update record failed');
    },
  });

  const deleteRecordMutation = useMutation({
    mutationFn: (id: string) => hbApi.records.delete(collectionName, id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['records', collectionName] });
      toast.success(t('records.deleted_success'));
    },
    onError: (err: {
      response?: { data?: { error?: { message?: string } } };
      message?: string;
    }) => {
      toast.error(err.response?.data?.error?.message || err.message || 'Delete record failed');
    },
  });

  const openCreateModal = () => {
    setEditingRecord(null);
    const initial: Record<string, unknown> = {};
    collection?.fields?.forEach((f) => {
      initial[f.name] = f.type === 'bool' ? false : '';
    });
    setFormData(initial);
    setIsModalOpen(true);
  };

  const openEditModal = (rec: RecordModel) => {
    setEditingRecord(rec);
    setFormData({ ...rec });
    setIsModalOpen(true);
  };

  const closeModal = () => {
    setIsModalOpen(false);
    setEditingRecord(null);
  };

  const handleSaveRecord = (e: React.FormEvent) => {
    e.preventDefault();
    if (editingRecord) {
      updateRecordMutation.mutate({ id: editingRecord.id, data: formData });
    } else {
      createRecordMutation.mutate(formData);
    }
  };

  const handleDeleteRecord = (id: string) => {
    if (window.confirm(t('records.delete_confirm'))) {
      deleteRecordMutation.mutate(id);
    }
  };

  const handleCopyJson = (obj: unknown) => {
    navigator.clipboard.writeText(JSON.stringify(obj, null, 2));
    toast.success(t('app.copied'));
  };

  const sseVariant =
    sseStatus === 'connected' ? 'success' : sseStatus === 'connecting' ? 'warning' : 'default';

  return (
    <AdminPageLayout
      tabTitle={collectionName}
      tabIcon="i-ph:table-bold text-sky-400"
      tabBadge={totalCount}
      breadcrumbs={[
        { label: t('collections.title'), href: '/collections', icon: 'i-ph:database-bold' },
        { label: collectionName, icon: 'i-ph:table-bold text-sky-400' },
      ]}
      actions={
        <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
          <JbStatusTag
            variant={sseVariant}
            icon={
              sseStatus === 'connected'
                ? 'i-ph:broadcast-bold'
                : sseStatus === 'connecting'
                  ? 'i-ph:spinner-gap-bold animate-spin'
                  : 'i-ph:cloud-slash-bold'
            }
          >
            {sseStatus === 'connected'
              ? t('records.live_active')
              : sseStatus === 'connecting'
                ? t('records.live_connecting')
                : t('records.live_offline')}
          </JbStatusTag>

          <Button
            onClick={() => refetch()}
            style={{ height: 24, fontSize: 11, display: 'flex', alignItems: 'center', gap: 4 }}
          >
            <span className="i-ph:arrow-clockwise-bold text-11px" />
            <span>{t('app.refresh')}</span>
          </Button>

          <Button
            primary
            onClick={openCreateModal}
            style={{ height: 24, fontSize: 11, display: 'flex', alignItems: 'center', gap: 4 }}
          >
            <span className="i-ph:plus-bold text-11px" />
            <span>{t('records.new')}</span>
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
          <div
            style={{
              position: 'relative',
              flex: 1,
              minWidth: 200,
              display: 'flex',
              alignItems: 'center',
            }}
          >
            <span className="i-ph:funnel-bold text-12px text-zinc-400 absolute left-2.5" />
            <input
              type="text"
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
              placeholder={t('records.filter_placeholder')}
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

          <div style={{ position: 'relative', width: 180, display: 'flex', alignItems: 'center' }}>
            <span className="i-ph:sort-ascending-bold text-12px text-zinc-400 absolute left-2.5" />
            <input
              type="text"
              value={sort}
              onChange={(e) => setSort(e.target.value)}
              placeholder={t('records.sort_placeholder')}
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

          <Button onClick={() => refetch()} style={{ height: 26, fontSize: 12 }}>
            <span className="i-ph:magnifying-glass-bold text-12px mr-1" />
            {t('app.search')}
          </Button>
        </div>

        {/* Records Data Table */}
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
                    padding: '8px 12px',
                    fontWeight: 600,
                    color: 'var(--jb-text-heading)',
                    width: 180,
                  }}
                >
                  id
                </th>
                {collection?.fields?.map((f) => (
                  <th
                    key={f.name}
                    style={{
                      padding: '8px 12px',
                      fontWeight: 600,
                      color: 'var(--jb-text-heading)',
                    }}
                  >
                    <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
                      <span>{f.name}</span>
                      <span
                        style={{ fontSize: 10, color: 'var(--jb-text-muted)', fontWeight: 400 }}
                      >
                        ({f.type})
                      </span>
                    </div>
                  </th>
                ))}
                <th
                  style={{
                    padding: '8px 12px',
                    fontWeight: 600,
                    color: 'var(--jb-text-heading)',
                    width: 140,
                  }}
                >
                  created_at
                </th>
                <th
                  style={{
                    padding: '8px 12px',
                    fontWeight: 600,
                    color: 'var(--jb-text-heading)',
                    width: 110,
                    textAlign: 'right',
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
                    colSpan={100}
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
              ) : records.length === 0 ? (
                <tr>
                  <td
                    colSpan={100}
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
                      <span className="i-ph:file-text-bold text-30px opacity-40" />
                      <span>{t('records.empty')}</span>
                    </div>
                  </td>
                </tr>
              ) : (
                records.map((rec) => (
                  <tr
                    key={rec.id}
                    style={{
                      borderBottom: '1px solid var(--jb-border)',
                      transition: 'background-color 0.15s',
                    }}
                    className="hover:bg-[var(--jb-active-item)]"
                  >
                    <td
                      style={{
                        padding: '8px 12px',
                        fontFamily: 'monospace',
                        color: 'var(--jb-accent-blue)',
                        fontWeight: 600,
                      }}
                    >
                      {rec.id}
                    </td>

                    {collection?.fields?.map((f) => {
                      const val = rec[f.name];
                      const isObj = typeof val === 'object' && val !== null;
                      return (
                        <td
                          key={f.name}
                          style={{
                            padding: '8px 12px',
                            maxWidth: 220,
                            overflow: 'hidden',
                            textOverflow: 'ellipsis',
                            whiteSpace: 'nowrap',
                          }}
                        >
                          {isObj ? (
                            <button
                              type="button"
                              onClick={() => setJsonViewModal(rec)}
                              style={{
                                background: 'var(--jb-panel-bg)',
                                border: '1px solid var(--jb-border)',
                                color: 'var(--jb-accent-purple)',
                                borderRadius: 4,
                                padding: '2px 6px',
                                fontSize: 11,
                                cursor: 'pointer',
                                display: 'inline-flex',
                                alignItems: 'center',
                                gap: 4,
                              }}
                            >
                              <span className="i-ph:code-bold text-12px" />
                              <span>JSON</span>
                            </button>
                          ) : typeof val === 'boolean' ? (
                            <JbStatusTag variant={val ? 'success' : 'error'} size="sm">
                              {val ? 'TRUE' : 'FALSE'}
                            </JbStatusTag>
                          ) : (
                            String(val ?? '')
                          )}
                        </td>
                      );
                    })}

                    <td
                      style={{ padding: '8px 12px', color: 'var(--jb-text-muted)', fontSize: 11 }}
                    >
                      {String(rec.created_at || rec.created || '-')}
                    </td>

                    <td style={{ padding: '8px 12px', textAlign: 'right' }}>
                      <div style={{ display: 'flex', gap: 6, justifyContent: 'flex-end' }}>
                        <button
                          type="button"
                          onClick={() => setJsonViewModal(rec)}
                          title={t('records.json_view')}
                          style={{
                            background: 'transparent',
                            border: '1px solid var(--jb-border)',
                            color: 'var(--jb-text)',
                            borderRadius: 4,
                            padding: '3px 6px',
                            fontSize: 11,
                            cursor: 'pointer',
                            display: 'flex',
                            alignItems: 'center',
                          }}
                        >
                          <span className="i-ph:code-bold" />
                        </button>
                        <button
                          type="button"
                          onClick={() => openEditModal(rec)}
                          title={t('app.edit')}
                          style={{
                            background: 'transparent',
                            border: '1px solid var(--jb-border)',
                            color: 'var(--jb-accent-blue)',
                            borderRadius: 4,
                            padding: '3px 6px',
                            fontSize: 11,
                            cursor: 'pointer',
                            display: 'flex',
                            alignItems: 'center',
                          }}
                        >
                          <span className="i-ph:pencil-simple-line-bold" />
                        </button>
                        <button
                          type="button"
                          onClick={() => handleDeleteRecord(rec.id)}
                          title={t('app.delete')}
                          style={{
                            background: 'transparent',
                            border: '1px solid var(--jb-border)',
                            color: '#ef4444',
                            borderRadius: 4,
                            padding: '3px 6px',
                            fontSize: 11,
                            cursor: 'pointer',
                            display: 'flex',
                            alignItems: 'center',
                          }}
                        >
                          <span className="i-ph:trash-bold" />
                        </button>
                      </div>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>

        {/* Pagination Footer */}
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
            {t('records.total_items', { total: totalCount })}
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

      {/* Create / Edit Record Modal */}
      <JbModal
        open={isModalOpen}
        onClose={closeModal}
        width={580}
        title={editingRecord ? `${t('records.edit')} (${editingRecord.id})` : t('records.new')}
        icon="i-ph:table-bold text-sky-400"
        footer={
          <>
            <Button onClick={closeModal} style={{ height: 28, fontSize: 12 }}>
              {t('app.cancel')}
            </Button>
            <Button
              primary
              onClick={handleSaveRecord}
              disabled={createRecordMutation.isPending || updateRecordMutation.isPending}
              style={{ height: 28, fontSize: 12, display: 'flex', alignItems: 'center', gap: 4 }}
            >
              {createRecordMutation.isPending || updateRecordMutation.isPending ? (
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
          onSubmit={handleSaveRecord}
          style={{ display: 'flex', flexDirection: 'column', gap: 14 }}
        >
          {collection?.fields?.map((field) => (
            <div key={field.name}>
              <label
                style={{
                  fontSize: 12,
                  fontWeight: 600,
                  color: 'var(--jb-text-muted)',
                  display: 'block',
                  marginBottom: 4,
                }}
              >
                {field.name} {field.required ? '*' : ''}
                <span style={{ fontSize: 11, fontWeight: 400, marginLeft: 4 }}>({field.type})</span>
              </label>

              {field.type === 'bool' ? (
                <label style={{ display: 'flex', alignItems: 'center', gap: 6, cursor: 'pointer' }}>
                  <input
                    type="checkbox"
                    checked={Boolean(formData[field.name])}
                    onChange={(e) => setFormData({ ...formData, [field.name]: e.target.checked })}
                  />
                  <span style={{ fontSize: 12 }}>{formData[field.name] ? 'True' : 'False'}</span>
                </label>
              ) : field.type === 'json' ? (
                <textarea
                  rows={4}
                  value={
                    typeof formData[field.name] === 'object'
                      ? JSON.stringify(formData[field.name], null, 2)
                      : String(formData[field.name] || '')
                  }
                  onChange={(e) => {
                    try {
                      setFormData({ ...formData, [field.name]: JSON.parse(e.target.value) });
                    } catch {
                      setFormData({ ...formData, [field.name]: e.target.value });
                    }
                  }}
                  style={{
                    width: '100%',
                    padding: '6px 10px',
                    borderRadius: 4,
                    border: '1px solid var(--jb-border)',
                    backgroundColor: 'var(--jb-editor-bg)',
                    color: 'var(--jb-text)',
                    fontSize: 12,
                    fontFamily: 'monospace',
                    boxSizing: 'border-box',
                  }}
                />
              ) : (
                <input
                  type={field.type === 'number' ? 'number' : 'text'}
                  value={String(formData[field.name] ?? '')}
                  onChange={(e) =>
                    setFormData({
                      ...formData,
                      [field.name]:
                        field.type === 'number' ? Number(e.target.value) : e.target.value,
                    })
                  }
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
              )}
            </div>
          ))}
        </form>
      </JbModal>

      {/* JSON Viewer Modal */}
      <JbModal
        open={Boolean(jsonViewModal)}
        onClose={() => setJsonViewModal(null)}
        width={600}
        title={`${t('records.json_view')} (${jsonViewModal?.id})`}
        icon="i-ph:code-bold text-purple-400"
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
              onClick={() => handleCopyJson(jsonViewModal)}
              style={{ height: 28, fontSize: 12, display: 'flex', alignItems: 'center', gap: 4 }}
            >
              <span className="i-ph:copy-bold text-12px" />
              <span>{t('app.copy')}</span>
            </Button>
            <Button onClick={() => setJsonViewModal(null)} style={{ height: 28, fontSize: 12 }}>
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
            maxHeight: 400,
            color: 'var(--jb-text)',
          }}
        >
          {JSON.stringify(jsonViewModal, null, 2)}
        </pre>
      </JbModal>
    </AdminPageLayout>
  );
}
