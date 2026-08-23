import Button from '@jetbrains/ring-ui-built/components/button/button';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { createFileRoute, Link, useNavigate } from '@tanstack/react-router';
import React, { useState } from 'react';

import { AdminPageLayout } from '../../../components/layout/AdminPageLayout';
import { JbCard } from '../../../components/ui/JbCard';
import { JbModal } from '../../../components/ui/JbModal';
import { JbStatusTag } from '../../../components/ui/JbStatusTag';
import { useToast } from '../../../components/ui/Toast';
import {
  hbApi,
  type CollectionModel,
  type FieldDef,
  type FieldTypeName,
  type CollectionRules,
} from '../../../lib/api';
import { useI18n } from '../../../lib/i18n';

export const Route = createFileRoute('/_admin/collections/')({
  component: CollectionsOverviewPage,
});

const DEFAULT_FIELD_TYPES: FieldTypeName[] = [
  'text',
  'number',
  'bool',
  'datetime',
  'json',
  'file',
  'relation',
  'select',
  'email',
  'url',
];

function CollectionsOverviewPage() {
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const { t } = useI18n();
  const toast = useToast();

  const [isModalOpen, setIsModalOpen] = useState(false);
  const [editingCollection, setEditingCollection] = useState<CollectionModel | null>(null);

  // Form State
  const [name, setName] = useState('');
  const [type, setType] = useState<'base' | 'auth'>('base');
  const [fields, setFields] = useState<FieldDef[]>([
    { name: 'title', type: 'text', required: true },
  ]);
  const [listRule, setListRule] = useState<string>('');
  const [viewRule, setViewRule] = useState<string>('');
  const [createRule, setCreateRule] = useState<string>('');
  const [updateRule, setUpdateRule] = useState<string>('');
  const [deleteRule, setDeleteRule] = useState<string>('');

  // Query Collections
  const {
    data: collectionsRes,
    isLoading,
    refetch,
  } = useQuery({
    queryKey: ['collections'],
    queryFn: async () => {
      const res = await hbApi.collections.list();
      return res.data.data || [];
    },
  });

  const collections = collectionsRes || [];

  // Mutations
  const createMutation = useMutation({
    mutationFn: (data: CollectionModel) => hbApi.collections.create(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['collections'] });
      toast.success(t('collections.created_success'));
      closeModal();
    },
    onError: (err: {
      response?: { data?: { error?: { message?: string } } };
      message?: string;
    }) => {
      toast.error(err.response?.data?.error?.message || err.message || 'Create failed');
    },
  });

  const updateMutation = useMutation({
    mutationFn: ({ colName, data }: { colName: string; data: Partial<CollectionModel> }) =>
      hbApi.collections.update(colName, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['collections'] });
      toast.success(t('collections.updated_success'));
      closeModal();
    },
    onError: (err: {
      response?: { data?: { error?: { message?: string } } };
      message?: string;
    }) => {
      toast.error(err.response?.data?.error?.message || err.message || 'Update failed');
    },
  });

  const deleteMutation = useMutation({
    mutationFn: (colName: string) => hbApi.collections.delete(colName),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['collections'] });
      toast.success(t('collections.deleted_success'));
    },
    onError: (err: {
      response?: { data?: { error?: { message?: string } } };
      message?: string;
    }) => {
      toast.error(err.response?.data?.error?.message || err.message || 'Delete failed');
    },
  });

  const handleSave = (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) {
      toast.warning(t('collections.name') + ' 不能为空');
      return;
    }

    const cleanedFields = fields
      .map((f) => ({ ...f, name: f.name.trim() }))
      .filter((f) => f.name.length > 0);

    const rules: CollectionRules = {
      list: listRule.trim() === '' ? null : listRule.trim(),
      view: viewRule.trim() === '' ? null : viewRule.trim(),
      create: createRule.trim() === '' ? null : createRule.trim(),
      update: updateRule.trim() === '' ? null : updateRule.trim(),
      delete: deleteRule.trim() === '' ? null : deleteRule.trim(),
    };

    if (editingCollection) {
      updateMutation.mutate({
        colName: editingCollection.name,
        data: {
          fields: cleanedFields,
          rules,
        },
      });
    } else {
      createMutation.mutate({
        name: name.trim(),
        type,
        fields: cleanedFields,
        rules,
      });
    }
  };

  const openCreateModal = () => {
    setEditingCollection(null);
    setName('');
    setType('base');
    setFields([{ name: 'title', type: 'text', required: true }]);
    setListRule('');
    setViewRule('');
    setCreateRule('');
    setUpdateRule('');
    setDeleteRule('');
    setIsModalOpen(true);
  };

  const openEditModal = (col: CollectionModel) => {
    setEditingCollection(col);
    setName(col.name);
    setType(col.type);
    setFields(
      col.fields && col.fields.length > 0
        ? [...col.fields]
        : [{ name: 'title', type: 'text', required: true }],
    );
    setListRule(
      col.rules?.list === null || col.rules?.list === undefined ? '' : String(col.rules.list),
    );
    setViewRule(
      col.rules?.view === null || col.rules?.view === undefined ? '' : String(col.rules.view),
    );
    setCreateRule(
      col.rules?.create === null || col.rules?.create === undefined ? '' : String(col.rules.create),
    );
    setUpdateRule(
      col.rules?.update === null || col.rules?.update === undefined ? '' : String(col.rules.update),
    );
    setDeleteRule(
      col.rules?.delete === null || col.rules?.delete === undefined ? '' : String(col.rules.delete),
    );
    setIsModalOpen(true);
  };

  const closeModal = () => {
    setIsModalOpen(false);
    setEditingCollection(null);
  };

  const handleAddField = () => {
    setFields([...fields, { name: '', type: 'text', required: false }]);
  };

  const handleRemoveField = (index: number) => {
    setFields(fields.filter((_, i) => i !== index));
  };

  const handleFieldChange = (index: number, key: keyof FieldDef, val: unknown) => {
    const next = [...fields];
    next[index] = { ...next[index], [key]: val };
    setFields(next);
  };

  const handleDelete = (colName: string) => {
    if (window.confirm(t('collections.delete_confirm', { name: colName }))) {
      deleteMutation.mutate(colName);
    }
  };

  return (
    <AdminPageLayout
      tabTitle={t('collections.title')}
      tabIcon="i-ph:database-bold text-amber-500"
      tabBadge={collections.length}
      breadcrumbs={[{ label: t('collections.title'), icon: 'i-ph:database-bold text-amber-500' }]}
      actions={
        <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
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
            <span>{t('collections.create')}</span>
          </Button>
        </div>
      }
      banner={
        <div className="jb-welcome-banner">
          <div className="jb-banner-info">
            <h2>{t('collections.title')}</h2>
            <p>{t('collections.subtitle')}</p>
            <div className="jb-banner-tags">
              <span className="jb-tag-blue">
                <span className="i-ph:table-bold" />
                <span>{t('collections.tables_count', { count: collections.length })}</span>
              </span>
              <span className="jb-tag-green">
                <span className="i-ph:lightning-bold" />
                <span>SurrealDB Schema Ready</span>
              </span>
            </div>
          </div>
          <div className="jb-banner-actions">
            <Button
              primary
              onClick={openCreateModal}
              style={{ height: 28, fontSize: 12, display: 'flex', alignItems: 'center', gap: 6 }}
            >
              <span className="i-ph:plus-circle-bold text-13px" />
              <span>{t('collections.create')}</span>
            </Button>
          </div>
        </div>
      }
    >
      <JbCard noBodyPadding style={{ overflowX: 'auto' }}>
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
                {t('collections.name')}
              </th>
              <th
                style={{ padding: '10px 16px', fontWeight: 600, color: 'var(--jb-text-heading)' }}
              >
                {t('collections.type')}
              </th>
              <th
                style={{ padding: '10px 16px', fontWeight: 600, color: 'var(--jb-text-heading)' }}
              >
                {t('collections.fields')}
              </th>
              <th
                style={{ padding: '10px 16px', fontWeight: 600, color: 'var(--jb-text-heading)' }}
              >
                {t('collections.rules')}
              </th>
              <th
                style={{
                  padding: '10px 16px',
                  fontWeight: 600,
                  color: 'var(--jb-text-heading)',
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
            ) : collections.length === 0 ? (
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
                      gap: 8,
                    }}
                  >
                    <span className="i-ph:database-bold text-32px opacity-40" />
                    <span>{t('collections.empty')}</span>
                  </div>
                </td>
              </tr>
            ) : (
              collections.map((col) => (
                <tr
                  key={col.name}
                  style={{
                    borderBottom: '1px solid var(--jb-border)',
                    transition: 'background-color 0.15s',
                  }}
                  className="hover:bg-[var(--jb-active-item)]"
                >
                  <td style={{ padding: '12px 16px' }}>
                    <Link
                      to="/collections/$collectionName"
                      params={{ collectionName: col.name }}
                      style={{
                        display: 'flex',
                        alignItems: 'center',
                        gap: 8,
                        color: 'var(--jb-text-heading)',
                        fontWeight: 600,
                        textDecoration: 'none',
                      }}
                    >
                      <span
                        className={
                          col.type === 'auth'
                            ? 'i-ph:shield-check-bold text-purple-400 text-16px'
                            : 'i-ph:table-bold text-sky-400 text-16px'
                        }
                      />
                      <span style={{ fontSize: 13 }}>{col.name}</span>
                    </Link>
                  </td>
                  <td style={{ padding: '12px 16px' }}>
                    <JbStatusTag
                      variant={col.type === 'auth' ? 'warning' : 'info'}
                      icon={col.type === 'auth' ? 'i-ph:shield-bold' : 'i-ph:table-bold'}
                    >
                      {col.type === 'auth'
                        ? t('collections.type.auth')
                        : t('collections.type.base')}
                    </JbStatusTag>
                  </td>
                  <td style={{ padding: '12px 16px', color: 'var(--jb-text-muted)' }}>
                    {col.fields?.map((f) => `${f.name} (${f.type})`).join(', ') ||
                      t('collections.no_fields')}
                  </td>
                  <td style={{ padding: '12px 16px', fontSize: 12, color: 'var(--jb-text-muted)' }}>
                    <span
                      title={`List: ${col.rules?.list ?? 'admin'}\nCreate: ${col.rules?.create ?? 'admin'}`}
                      style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}
                    >
                      {col.rules?.list === null || col.rules?.list === undefined ? (
                        <>
                          <span className="i-ph:lock-simple-bold text-amber-500 text-13px" />
                          <span>{t('collections.rule.admin_only')}</span>
                        </>
                      ) : (
                        <>
                          <span className="i-ph:lock-simple-open-bold text-emerald-500 text-13px" />
                          <span>{t('collections.rule.configured')}</span>
                        </>
                      )}
                    </span>
                  </td>
                  <td style={{ padding: '12px 16px', textAlign: 'right' }}>
                    <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end' }}>
                      <Button
                        onClick={() =>
                          navigate({
                            to: '/collections/$collectionName',
                            params: { collectionName: col.name },
                          })
                        }
                        style={{
                          height: 26,
                          fontSize: 12,
                          display: 'flex',
                          alignItems: 'center',
                          gap: 4,
                        }}
                      >
                        <span className="i-ph:list-dashes-bold text-12px" />
                        <span>{t('records.title')}</span>
                      </Button>
                      <Button
                        onClick={() => openEditModal(col)}
                        style={{
                          height: 26,
                          fontSize: 12,
                          display: 'flex',
                          alignItems: 'center',
                          gap: 4,
                        }}
                      >
                        <span className="i-ph:pencil-simple-bold text-12px" />
                        <span>{t('app.edit')}</span>
                      </Button>
                      <Button
                        onClick={() => handleDelete(col.name)}
                        style={{
                          height: 26,
                          fontSize: 12,
                          color: '#ef4444',
                          display: 'flex',
                          alignItems: 'center',
                          gap: 4,
                        }}
                      >
                        <span className="i-ph:trash-bold text-12px" />
                        <span>{t('app.delete')}</span>
                      </Button>
                    </div>
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </JbCard>

      {/* Modal for Create/Edit Collection */}
      <JbModal
        open={isModalOpen}
        onClose={closeModal}
        width={680}
        title={
          editingCollection
            ? `${t('app.edit')}: ${editingCollection.name}`
            : t('collections.create')
        }
        icon="i-ph:database-bold text-blue-500"
        footer={
          <>
            <Button onClick={closeModal} style={{ height: 28, fontSize: 12 }}>
              {t('app.cancel')}
            </Button>
            <Button
              primary
              onClick={handleSave}
              disabled={createMutation.isPending || updateMutation.isPending}
              style={{ height: 28, fontSize: 12, display: 'flex', alignItems: 'center', gap: 4 }}
            >
              {createMutation.isPending || updateMutation.isPending ? (
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
        <form onSubmit={handleSave} style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
          {/* Name & Type */}
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 14 }}>
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
                {t('collections.name')} *
              </label>
              <input
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                disabled={Boolean(editingCollection)}
                placeholder="posts"
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
              <div style={{ fontSize: 11, color: 'var(--jb-text-muted)', marginTop: 2 }}>
                {t('collections.name_help')}
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
                {t('collections.type')}
              </label>
              <select
                value={type}
                onChange={(e) => setType(e.target.value as 'base' | 'auth')}
                disabled={Boolean(editingCollection)}
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
                <option value="base">{t('collections.type.base')}</option>
                <option value="auth">{t('collections.type.auth')}</option>
              </select>
            </div>
          </div>

          {/* Fields Builder */}
          <div>
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                marginBottom: 8,
              }}
            >
              <label
                style={{
                  fontSize: 12,
                  fontWeight: 700,
                  color: 'var(--jb-text-heading)',
                  display: 'flex',
                  alignItems: 'center',
                  gap: 6,
                }}
              >
                <span className="i-ph:list-bullets-bold text-sky-400 text-14px" />
                <span>{t('collections.fields')}</span>
              </label>
              <Button
                onClick={handleAddField}
                style={{
                  height: 24,
                  fontSize: 11,
                  padding: '0 8px',
                  display: 'flex',
                  alignItems: 'center',
                  gap: 4,
                }}
              >
                <span className="i-ph:plus-bold text-10px" />
                <span>{t('collections.add_field')}</span>
              </Button>
            </div>

            <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
              {fields.map((field, idx) => (
                <div
                  key={idx}
                  style={{
                    display: 'grid',
                    gridTemplateColumns: '2fr 2fr 1fr auto',
                    gap: 8,
                    alignItems: 'center',
                    background: 'var(--jb-header-bg)',
                    padding: '6px 10px',
                    borderRadius: 4,
                    border: '1px solid var(--jb-border)',
                  }}
                >
                  <input
                    type="text"
                    placeholder={t('collections.field_name')}
                    value={field.name}
                    onChange={(e) => handleFieldChange(idx, 'name', e.target.value)}
                    style={{
                      padding: '4px 8px',
                      borderRadius: 3,
                      border: '1px solid var(--jb-border)',
                      backgroundColor: 'var(--jb-editor-bg)',
                      color: 'var(--jb-text)',
                      fontSize: 12,
                    }}
                  />

                  <select
                    value={field.type}
                    onChange={(e) =>
                      handleFieldChange(idx, 'type', e.target.value as FieldTypeName)
                    }
                    style={{
                      padding: '4px 8px',
                      borderRadius: 3,
                      border: '1px solid var(--jb-border)',
                      backgroundColor: 'var(--jb-editor-bg)',
                      color: 'var(--jb-text)',
                      fontSize: 12,
                    }}
                  >
                    {DEFAULT_FIELD_TYPES.map((ft) => (
                      <option key={ft} value={ft}>
                        {ft}
                      </option>
                    ))}
                  </select>

                  <label
                    style={{
                      display: 'flex',
                      alignItems: 'center',
                      gap: 4,
                      fontSize: 11,
                      cursor: 'pointer',
                    }}
                  >
                    <input
                      type="checkbox"
                      checked={field.required || false}
                      onChange={(e) => handleFieldChange(idx, 'required', e.target.checked)}
                    />
                    <span>{t('collections.field_required')}</span>
                  </label>

                  <button
                    type="button"
                    onClick={() => handleRemoveField(idx)}
                    style={{
                      background: 'transparent',
                      border: 'none',
                      color: '#ef4444',
                      cursor: 'pointer',
                      display: 'flex',
                      padding: 2,
                    }}
                  >
                    <span className="i-ph:trash-bold text-14px" />
                  </button>
                </div>
              ))}
            </div>
          </div>

          {/* API Rules Builder */}
          <div>
            <label
              style={{
                fontSize: 12,
                fontWeight: 700,
                color: 'var(--jb-text-heading)',
                display: 'flex',
                alignItems: 'center',
                gap: 6,
                marginBottom: 4,
              }}
            >
              <span className="i-ph:shield-check-bold text-purple-400 text-14px" />
              <span>{t('collections.rules')}</span>
            </label>
            <div style={{ fontSize: 11, color: 'var(--jb-text-muted)', marginBottom: 8 }}>
              {t('collections.rule.help')}
            </div>

            <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
              <div>
                <span
                  style={{
                    fontSize: 11,
                    color: 'var(--jb-text-muted)',
                    display: 'block',
                    marginBottom: 2,
                  }}
                >
                  {t('collections.rule.list')}
                </span>
                <input
                  type="text"
                  placeholder={t('collections.rule.placeholder_public')}
                  value={listRule}
                  onChange={(e) => setListRule(e.target.value)}
                  style={{
                    width: '100%',
                    padding: '5px 8px',
                    borderRadius: 3,
                    border: '1px solid var(--jb-border)',
                    backgroundColor: 'var(--jb-editor-bg)',
                    color: 'var(--jb-text)',
                    fontSize: 12,
                    boxSizing: 'border-box',
                  }}
                />
              </div>

              <div>
                <span
                  style={{
                    fontSize: 11,
                    color: 'var(--jb-text-muted)',
                    display: 'block',
                    marginBottom: 2,
                  }}
                >
                  {t('collections.rule.view')}
                </span>
                <input
                  type="text"
                  placeholder={t('collections.rule.placeholder_public')}
                  value={viewRule}
                  onChange={(e) => setViewRule(e.target.value)}
                  style={{
                    width: '100%',
                    padding: '5px 8px',
                    borderRadius: 3,
                    border: '1px solid var(--jb-border)',
                    backgroundColor: 'var(--jb-editor-bg)',
                    color: 'var(--jb-text)',
                    fontSize: 12,
                    boxSizing: 'border-box',
                  }}
                />
              </div>

              <div>
                <span
                  style={{
                    fontSize: 11,
                    color: 'var(--jb-text-muted)',
                    display: 'block',
                    marginBottom: 2,
                  }}
                >
                  {t('collections.rule.create')}
                </span>
                <input
                  type="text"
                  placeholder={t('collections.rule.placeholder_admin')}
                  value={createRule}
                  onChange={(e) => setCreateRule(e.target.value)}
                  style={{
                    width: '100%',
                    padding: '5px 8px',
                    borderRadius: 3,
                    border: '1px solid var(--jb-border)',
                    backgroundColor: 'var(--jb-editor-bg)',
                    color: 'var(--jb-text)',
                    fontSize: 12,
                    boxSizing: 'border-box',
                  }}
                />
              </div>

              <div>
                <span
                  style={{
                    fontSize: 11,
                    color: 'var(--jb-text-muted)',
                    display: 'block',
                    marginBottom: 2,
                  }}
                >
                  {t('collections.rule.update')}
                </span>
                <input
                  type="text"
                  placeholder={t('collections.rule.placeholder_admin')}
                  value={updateRule}
                  onChange={(e) => setUpdateRule(e.target.value)}
                  style={{
                    width: '100%',
                    padding: '5px 8px',
                    borderRadius: 3,
                    border: '1px solid var(--jb-border)',
                    backgroundColor: 'var(--jb-editor-bg)',
                    color: 'var(--jb-text)',
                    fontSize: 12,
                    boxSizing: 'border-box',
                  }}
                />
              </div>

              <div>
                <span
                  style={{
                    fontSize: 11,
                    color: 'var(--jb-text-muted)',
                    display: 'block',
                    marginBottom: 2,
                  }}
                >
                  {t('collections.rule.delete')}
                </span>
                <input
                  type="text"
                  placeholder={t('collections.rule.placeholder_admin')}
                  value={deleteRule}
                  onChange={(e) => setDeleteRule(e.target.value)}
                  style={{
                    width: '100%',
                    padding: '5px 8px',
                    borderRadius: 3,
                    border: '1px solid var(--jb-border)',
                    backgroundColor: 'var(--jb-editor-bg)',
                    color: 'var(--jb-text)',
                    fontSize: 12,
                    boxSizing: 'border-box',
                  }}
                />
              </div>
            </div>
          </div>
        </form>
      </JbModal>
    </AdminPageLayout>
  );
}
