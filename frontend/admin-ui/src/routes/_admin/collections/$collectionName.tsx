import Button from '@jetbrains/ring-ui-built/components/button/button';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { createFileRoute, Link } from '@tanstack/react-router';
import { useState } from 'react';

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
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [jsonViewModal, setJsonViewModal] = useState<RecordModel | null>(null);

  // Realtime Events Flash Notification
  const [realtimeNotify, setRealtimeNotify] = useState<{ action: string; time: string } | null>(
    null,
  );

  // 1. Fetch Collection Schema (GET /_/collections/{name})
  const { data: collectionRes } = useQuery({
    queryKey: ['collection', collectionName],
    queryFn: async () => {
      const res = await hbApi.collections.get(collectionName);
      return res.data.data;
    },
  });

  const collection: CollectionModel | undefined = collectionRes;

  // 2. Fetch Records with Filter & Pagination (GET /api/collections/{name}/records)
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
  const totalCount = recordsRes?.meta?.total ?? recordsRes?.meta?.totalItems ?? records.length;
  const totalPages = recordsRes?.meta?.totalPages ?? Math.max(1, Math.ceil(totalCount / perPage));

  // 3. SSE Realtime Subscription
  const { status: sseStatus } = useRealtimeCollection(collectionName, (event: RealtimeEvent) => {
    queryClient.invalidateQueries({ queryKey: ['records', collectionName] });
    setRealtimeNotify({
      action: `${event.action.toUpperCase()} (${new Date().toLocaleTimeString()})`,
      time: new Date().toLocaleTimeString(),
    });
    setTimeout(() => {
      setRealtimeNotify(null);
    }, 4000);
  });

  // 4. Create / Update Record Mutation
  const saveMutation = useMutation({
    mutationFn: async () => {
      if (editingRecord) {
        return hbApi.records.update(collectionName, editingRecord.id, formData);
      } else {
        return hbApi.records.create(collectionName, formData);
      }
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['records', collectionName] });
      closeRecordModal();
    },
    onError: (err: unknown) => {
      const axiosErr = err as {
        response?: { data?: { error?: { message?: string } } };
        message?: string;
      };
      setErrorMessage(
        axiosErr.response?.data?.error?.message || axiosErr.message || '保存记录失败',
      );
    },
  });

  // 5. Delete Record Mutation
  const deleteMutation = useMutation({
    mutationFn: async (id: string) => {
      return hbApi.records.delete(collectionName, id);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['records', collectionName] });
    },
  });

  const openCreateModal = () => {
    setEditingRecord(null);
    const initialData: Record<string, unknown> = {};
    collection?.fields?.forEach((f) => {
      if (f.type === 'bool') initialData[f.name] = false;
      else if (f.type === 'number') initialData[f.name] = 0;
      else if (f.type === 'json') initialData[f.name] = '{}';
      else initialData[f.name] = '';
    });
    setFormData(initialData);
    setErrorMessage(null);
    setIsModalOpen(true);
  };

  const openEditModal = (rec: RecordModel) => {
    setEditingRecord(rec);
    const cloneData = { ...rec };
    collection?.fields?.forEach((f) => {
      if (f.type === 'json' && typeof cloneData[f.name] === 'object') {
        cloneData[f.name] = JSON.stringify(cloneData[f.name], null, 2);
      }
    });
    setFormData(cloneData);
    setErrorMessage(null);
    setIsModalOpen(true);
  };

  const closeRecordModal = () => {
    setIsModalOpen(false);
    setEditingRecord(null);
    setFormData({});
    setErrorMessage(null);
  };

  const handleDeleteRecord = (id: string) => {
    if (window.confirm(t('records.delete_confirm'))) {
      deleteMutation.mutate(id);
    }
  };

  const handleFormFieldChange = (fieldName: string, value: unknown) => {
    setFormData((prev) => ({ ...prev, [fieldName]: value }));
  };

  const schemaFields = collection?.fields || [];

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
        <div style={{ display: 'flex', alignItems: 'center' }}>
          <Link
            to="/collections"
            title="Back to Collections"
            style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              width: 24,
              height: 24,
              color: 'var(--jb-text-muted)',
              textDecoration: 'none',
              borderRadius: 4,
              marginLeft: 4,
              marginRight: 4,
              cursor: 'pointer',
            }}
            className="hover:bg-[var(--jb-hover-item)] hover:text-[var(--jb-accent-blue)]"
          >
            <span className="i-ph:arrow-left-bold text-12px" />
          </Link>
          <div className="jb-editor-tab active">
            <span
              className={
                collection?.type === 'auth'
                  ? 'i-ph:shield-check-bold text-purple-400 text-13px'
                  : 'i-ph:table-bold text-sky-400 text-13px'
              }
            />
            <span style={{ fontWeight: 500 }}>{collectionName}</span>
            <span
              style={{
                fontSize: 10,
                color: 'var(--jb-text-muted)',
                background: 'var(--jb-border)',
                padding: '1px 5px',
                borderRadius: 4,
              }}
            >
              {totalCount}
            </span>
          </div>
        </div>

        {/* SSE Live Status & Actions */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
          {/* Realtime Live Beacon */}
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 6,
              fontSize: 11,
              padding: '2px 8px',
              borderRadius: 12,
              background:
                sseStatus === 'connected' ? 'rgba(73, 156, 84, 0.15)' : 'rgba(214, 121, 41, 0.15)',
              color: sseStatus === 'connected' ? '#499c54' : '#d67929',
              fontWeight: 600,
            }}
          >
            <span
              style={{
                width: 6,
                height: 6,
                borderRadius: '50%',
                backgroundColor: sseStatus === 'connected' ? '#499c54' : '#d67929',
                boxShadow: sseStatus === 'connected' ? '0 0 6px #499c54' : 'none',
              }}
            />
            <span>
              {sseStatus === 'connected'
                ? t('records.live_active')
                : sseStatus === 'connecting'
                  ? t('records.live_connecting')
                  : t('records.live_offline')}
            </span>
          </div>

          {realtimeNotify && (
            <span
              style={{
                fontSize: 11,
                padding: '2px 8px',
                background: 'var(--jb-accent-blue)',
                color: '#fff',
                borderRadius: 4,
                display: 'flex',
                alignItems: 'center',
                gap: 4,
              }}
            >
              <span className="i-ph:lightning-bold text-12px" />
              <span>{realtimeNotify.action}</span>
            </span>
          )}

          <Button
            primary
            onClick={openCreateModal}
            style={{ height: 26, fontSize: 12, display: 'flex', alignItems: 'center', gap: 4 }}
          >
            <span className="i-ph:plus-bold text-11px" />
            <span>{t('records.new')}</span>
          </Button>
        </div>
      </div>

      {/* Breadcrumbs */}
      <div className="jb-breadcrumbs">
        <span>HertaBase</span>
        <span className="jb-breadcrumb-sep">›</span>
        <span>Schema</span>
        <span className="jb-breadcrumb-sep">›</span>
        <Link
          to="/collections"
          className="jb-breadcrumb-link"
          style={{ textDecoration: 'none', color: 'inherit' }}
        >
          <span className="i-ph:table-bold text-11px text-zinc-400" />
          <span>{t('collections.title')}</span>
        </Link>
        <span className="jb-breadcrumb-sep">›</span>
        <span style={{ color: 'var(--jb-accent-blue)', fontWeight: 500 }}>{collectionName}</span>
      </div>

      {/* Query Filter Toolbar */}
      <div
        style={{
          padding: '8px 16px',
          background: 'var(--jb-header-bg)',
          borderBottom: '1px solid var(--jb-border)',
          display: 'flex',
          gap: 10,
          alignItems: 'center',
          flexWrap: 'wrap',
        }}
      >
        <div
          style={{
            flex: 2,
            minWidth: 200,
            display: 'flex',
            alignItems: 'center',
            position: 'relative',
          }}
        >
          <span className="i-ph:funnel-bold text-13px text-zinc-400 absolute left-3" />
          <input
            type="text"
            placeholder={t('records.filter_placeholder')}
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && refetch()}
            style={{
              width: '100%',
              padding: '5px 8px 5px 28px',
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
            flex: 1,
            minWidth: 140,
            display: 'flex',
            alignItems: 'center',
            position: 'relative',
          }}
        >
          <span className="i-ph:sort-ascending-bold text-13px text-zinc-400 absolute left-3" />
          <input
            type="text"
            placeholder={t('records.sort_placeholder')}
            value={sort}
            onChange={(e) => setSort(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && refetch()}
            style={{
              width: '100%',
              padding: '5px 8px 5px 28px',
              borderRadius: 4,
              border: '1px solid var(--jb-border)',
              backgroundColor: 'var(--jb-editor-bg)',
              color: 'var(--jb-text)',
              fontSize: 12,
              boxSizing: 'border-box',
            }}
          />
        </div>

        <Button
          onClick={() => refetch()}
          style={{ height: 26, fontSize: 12, display: 'flex', alignItems: 'center', gap: 4 }}
        >
          <span className="i-ph:magnifying-glass-bold text-12px" />
          <span>{t('app.search')}</span>
        </Button>
      </div>

      {/* Data Grid Table View */}
      <div style={{ flex: 1, overflow: 'auto' }}>
        <table
          style={{ width: '100%', borderCollapse: 'collapse', fontSize: 12, textAlign: 'left' }}
        >
          <thead>
            <tr
              style={{
                backgroundColor: 'var(--jb-header-bg)',
                borderBottom: '1px solid var(--jb-border)',
                position: 'sticky',
                top: 0,
                zIndex: 2,
              }}
            >
              <th
                style={{
                  padding: '8px 12px',
                  fontWeight: 600,
                  color: 'var(--jb-text-heading)',
                  width: 140,
                }}
              >
                ID
              </th>
              {schemaFields.map((field) => (
                <th
                  key={field.name}
                  style={{ padding: '8px 12px', fontWeight: 600, color: 'var(--jb-text-heading)' }}
                >
                  {field.name}
                  <span style={{ fontSize: 10, color: 'var(--jb-text-muted)', marginLeft: 4 }}>
                    ({field.type})
                  </span>
                </th>
              ))}
              <th
                style={{
                  padding: '8px 12px',
                  fontWeight: 600,
                  color: 'var(--jb-text-heading)',
                  width: 150,
                }}
              >
                created_at
              </th>
              <th
                style={{
                  padding: '8px 12px',
                  fontWeight: 600,
                  color: 'var(--jb-text-heading)',
                  width: 120,
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
                  colSpan={schemaFields.length + 3}
                  style={{ padding: 28, textAlign: 'center', color: 'var(--jb-text-muted)' }}
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
                  colSpan={schemaFields.length + 3}
                  style={{ padding: 36, textAlign: 'center', color: 'var(--jb-text-muted)' }}
                >
                  <div
                    style={{
                      display: 'flex',
                      flexDirection: 'column',
                      alignItems: 'center',
                      gap: 8,
                    }}
                  >
                    <span className="i-ph:files-bold text-32px text-zinc-500" />
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
                >
                  <td
                    style={{
                      padding: '8px 12px',
                      fontFamily: 'JetBrains Mono, monospace',
                      color: 'var(--jb-accent-blue)',
                      fontWeight: 600,
                    }}
                  >
                    {rec.id}
                  </td>

                  {schemaFields.map((f) => {
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
                          <span
                            style={{
                              color: val ? '#499c54' : '#e53935',
                              fontWeight: 600,
                              display: 'inline-flex',
                              alignItems: 'center',
                              gap: 3,
                            }}
                          >
                            <span
                              className={
                                val ? 'i-ph:check-bold text-11px' : 'i-ph:x-bold text-11px'
                              }
                            />
                            <span>{val ? 'TRUE' : 'FALSE'}</span>
                          </span>
                        ) : (
                          String(val ?? '')
                        )}
                      </td>
                    );
                  })}

                  <td style={{ padding: '8px 12px', color: 'var(--jb-text-muted)', fontSize: 11 }}>
                    {String(rec.created_at || rec.created || '-')}
                  </td>

                  <td style={{ padding: '8px 12px', textAlign: 'right' }}>
                    <div style={{ display: 'flex', gap: 6, justifyContent: 'flex-end' }}>
                      <button
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
                        onClick={() => handleDeleteRecord(rec.id)}
                        title={t('app.delete')}
                        style={{
                          background: 'transparent',
                          border: '1px solid var(--jb-border)',
                          color: '#e53935',
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

      {/* Create / Edit Record Modal */}
      {isModalOpen && (
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
              width: 580,
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
            <div
              style={{
                padding: '12px 18px',
                borderBottom: '1px solid var(--jb-border)',
                background: 'var(--jb-header-bg)',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
              }}
            >
              <h3
                style={{
                  margin: 0,
                  fontSize: 14,
                  fontWeight: 700,
                  color: 'var(--jb-text-heading)',
                  display: 'flex',
                  alignItems: 'center',
                  gap: 6,
                }}
              >
                <span className="i-ph:file-plus-bold text-blue-500 text-15px" />
                <span>
                  {editingRecord ? `${t('records.edit')} (${editingRecord.id})` : t('records.new')}
                </span>
              </h3>
              <button
                onClick={closeRecordModal}
                style={{
                  background: 'transparent',
                  border: 'none',
                  color: 'var(--jb-text-muted)',
                  cursor: 'pointer',
                  display: 'flex',
                  padding: 2,
                }}
              >
                <span className="i-ph:x-bold text-15px" />
              </button>
            </div>

            <div
              style={{
                flex: 1,
                overflowY: 'auto',
                padding: 18,
                display: 'flex',
                flexDirection: 'column',
                gap: 14,
              }}
            >
              {errorMessage && (
                <div
                  style={{
                    padding: '8px 12px',
                    background: 'rgba(229,57,53,0.12)',
                    border: '1px solid #e53935',
                    color: '#e53935',
                    borderRadius: 6,
                    fontSize: 12,
                    display: 'flex',
                    alignItems: 'center',
                    gap: 8,
                  }}
                >
                  <span className="i-ph:warning-circle-bold text-15px shrink-0" />
                  <span>{errorMessage}</span>
                </div>
              )}

              {schemaFields.length === 0 ? (
                <div style={{ fontSize: 12, color: 'var(--jb-text-muted)', fontStyle: 'italic' }}>
                  当前集合未配置自定义字段，直接提交将创建空记录。
                </div>
              ) : (
                schemaFields.map((field) => (
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
                      {field.name} {field.required && <span style={{ color: '#e53935' }}>*</span>}
                      <span style={{ fontSize: 10, color: 'var(--jb-text-muted)', marginLeft: 6 }}>
                        ({field.type})
                      </span>
                    </label>

                    {field.type === 'bool' ? (
                      <label
                        style={{
                          display: 'flex',
                          alignItems: 'center',
                          gap: 6,
                          fontSize: 12,
                          cursor: 'pointer',
                        }}
                      >
                        <input
                          type="checkbox"
                          checked={Boolean(formData[field.name])}
                          onChange={(e) => handleFormFieldChange(field.name, e.target.checked)}
                        />
                        <span>{Boolean(formData[field.name]) ? 'True' : 'False'}</span>
                      </label>
                    ) : field.type === 'json' ? (
                      <textarea
                        rows={4}
                        value={String(formData[field.name] ?? '')}
                        onChange={(e) => handleFormFieldChange(field.name, e.target.value)}
                        placeholder='{"key": "value"}'
                        style={{
                          width: '100%',
                          padding: '6px 10px',
                          borderRadius: 4,
                          border: '1px solid var(--jb-border)',
                          backgroundColor: 'var(--jb-editor-bg)',
                          color: 'var(--jb-text)',
                          fontFamily: 'JetBrains Mono, monospace',
                          fontSize: 12,
                          boxSizing: 'border-box',
                        }}
                      />
                    ) : (
                      <input
                        type={field.type === 'number' ? 'number' : 'text'}
                        value={String(formData[field.name] ?? '')}
                        onChange={(e) =>
                          handleFormFieldChange(
                            field.name,
                            field.type === 'number'
                              ? e.target.value === ''
                                ? ''
                                : Number(e.target.value)
                              : e.target.value,
                          )
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
                ))
              )}
            </div>

            <div
              style={{
                padding: '10px 18px',
                borderTop: '1px solid var(--jb-border)',
                background: 'var(--jb-header-bg)',
                display: 'flex',
                justifyContent: 'flex-end',
                gap: 10,
              }}
            >
              <Button onClick={closeRecordModal} style={{ height: 30 }}>
                {t('app.cancel')}
              </Button>
              <Button
                primary
                onClick={() => saveMutation.mutate()}
                disabled={saveMutation.isPending}
                style={{
                  height: 30,
                  backgroundColor: 'var(--jb-accent-blue)',
                  color: '#fff',
                  display: 'flex',
                  alignItems: 'center',
                  gap: 4,
                }}
              >
                {saveMutation.isPending ? (
                  <>
                    <span className="i-ph:spinner-gap-bold animate-spin text-13px" />
                    <span>{t('app.loading')}</span>
                  </>
                ) : (
                  <>
                    <span className="i-ph:check-bold text-13px" />
                    <span>{t('app.save')}</span>
                  </>
                )}
              </Button>
            </div>
          </div>
        </div>
      )}

      {/* JSON Viewer Modal */}
      {jsonViewModal && (
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
              width: 540,
              maxHeight: '80vh',
              background: 'var(--jb-panel-bg)',
              border: '1px solid var(--jb-border)',
              borderRadius: 8,
              boxShadow: 'var(--jb-shadow)',
              display: 'flex',
              flexDirection: 'column',
              overflow: 'hidden',
            }}
          >
            <div
              style={{
                padding: '10px 16px',
                background: 'var(--jb-header-bg)',
                borderBottom: '1px solid var(--jb-border)',
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
              }}
            >
              <span
                style={{
                  fontSize: 13,
                  fontWeight: 700,
                  display: 'flex',
                  alignItems: 'center',
                  gap: 6,
                }}
              >
                <span className="i-ph:code-bold text-purple-400 text-14px" />
                <span>{t('records.json_view')}</span>
              </span>
              <button
                onClick={() => setJsonViewModal(null)}
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
            <div style={{ flex: 1, overflowY: 'auto', padding: 14 }}>
              <pre
                style={{
                  margin: 0,
                  fontFamily: 'JetBrains Mono, monospace',
                  fontSize: 12,
                  color: 'var(--jb-text)',
                  background: 'var(--jb-dock-bg)',
                  padding: 12,
                  borderRadius: 6,
                  border: '1px solid var(--jb-border)',
                }}
              >
                {JSON.stringify(jsonViewModal, null, 2)}
              </pre>
            </div>
            <div
              style={{
                padding: '8px 16px',
                background: 'var(--jb-header-bg)',
                borderTop: '1px solid var(--jb-border)',
                textAlign: 'right',
              }}
            >
              <Button onClick={() => setJsonViewModal(null)} style={{ height: 28 }}>
                {t('app.close')}
              </Button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
