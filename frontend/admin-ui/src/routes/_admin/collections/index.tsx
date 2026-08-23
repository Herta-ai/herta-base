import Button from '@jetbrains/ring-ui-built/components/button/button';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { createFileRoute, Link, useNavigate } from '@tanstack/react-router';
import { useState } from 'react';

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

  const [isModalOpen, setIsModalOpen] = useState(false);
  const [editingCollection, setEditingCollection] = useState<CollectionModel | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

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

  // Query Collections (From /_/collections)
  const { data: collectionsRes, isLoading } = useQuery({
    queryKey: ['collections'],
    queryFn: async () => {
      const res = await hbApi.collections.list();
      return res.data.data || [];
    },
  });

  const collections = collectionsRes || [];

  // Helper to parse rules input to boolean/string/null
  const parseRuleValue = (val: string): string | boolean | null => {
    const trimmed = val.trim();
    if (!trimmed) return null;
    if (trimmed === 'true') return true;
    if (trimmed === 'false') return false;
    return trimmed;
  };

  // Create / Update Mutation
  const saveMutation = useMutation({
    mutationFn: async () => {
      const rules: CollectionRules = {
        list: parseRuleValue(listRule),
        view: parseRuleValue(viewRule),
        create: parseRuleValue(createRule),
        update: parseRuleValue(updateRule),
        delete: parseRuleValue(deleteRule),
      };

      const validFields = fields.filter((f) => f.name.trim() !== '');

      if (editingCollection) {
        // PATCH /_/collections/{name}
        return hbApi.collections.update(editingCollection.name, {
          fields: validFields,
          indexes: editingCollection.indexes || [],
          rules,
        });
      } else {
        // POST /_/collections
        const payload: CollectionModel = {
          name,
          type,
          schema_mode: 'strict',
          fields: validFields,
          indexes: [],
          rules,
        };
        return hbApi.collections.create(payload);
      }
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['collections'] });
      closeModal();
    },
    onError: (err: unknown) => {
      const axiosErr = err as {
        response?: { data?: { error?: { message?: string } } };
        message?: string;
      };
      setErrorMessage(axiosErr.response?.data?.error?.message || axiosErr.message || '保存失败');
    },
  });

  // Delete Mutation
  const deleteMutation = useMutation({
    mutationFn: async (colName: string) => {
      return hbApi.collections.delete(colName);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['collections'] });
    },
  });

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
    setErrorMessage(null);
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
    setErrorMessage(null);
    setIsModalOpen(true);
  };

  const closeModal = () => {
    setIsModalOpen(false);
    setEditingCollection(null);
    setErrorMessage(null);
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
      <div className="jb-editor-tabs" style={{ justifyContent: 'space-between', paddingRight: 10 }}>
        <div style={{ display: 'flex' }}>
          <div className="jb-editor-tab active">
            <span className="i-ph:database-bold text-amber-500 text-13px" />
            <span>{t('collections.title')}</span>
          </div>
        </div>

        <Button
          primary
          onClick={openCreateModal}
          style={{ height: 26, fontSize: 12, display: 'flex', alignItems: 'center', gap: 4 }}
        >
          <span className="i-ph:plus-bold text-12px" />
          <span>{t('collections.create')}</span>
        </Button>
      </div>

      {/* Breadcrumbs */}
      <div className="jb-breadcrumbs">
        <span>HertaBase</span>
        <span className="jb-breadcrumb-sep">›</span>
        <span>Schema</span>
        <span className="jb-breadcrumb-sep">›</span>
        <span
          style={{
            color: 'var(--jb-accent-blue)',
            fontWeight: 500,
            display: 'inline-flex',
            alignItems: 'center',
            gap: 3,
          }}
        >
          <span className="i-ph:table-bold text-11px text-sky-400" />
          <span>{t('collections.title')}</span>
        </span>
      </div>

      {/* Main Content Area */}
      <div style={{ flex: 1, overflowY: 'auto', padding: 20 }}>
        {/* Welcome / Header Banner */}
        <div className="jb-welcome-banner" style={{ marginBottom: 20 }}>
          <div className="jb-banner-info">
            <h2>{t('collections.title')}</h2>
            <p>{t('collections.subtitle')}</p>
          </div>
          <div style={{ display: 'flex', gap: 8 }}>
            <span
              className="jb-branch-badge"
              style={{ display: 'flex', alignItems: 'center', gap: 4 }}
            >
              <span className="i-ph:table-bold text-12px" />
              <span>{t('collections.tables_count', { count: collections.length })}</span>
            </span>
          </div>
        </div>

        {/* Collections Table Grid */}
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
            style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13, textAlign: 'left' }}
          >
            <thead>
              <tr
                style={{
                  backgroundColor: 'var(--jb-header-bg)',
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
              ) : collections.length === 0 ? (
                <tr>
                  <td
                    colSpan={5}
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
                      <span className="i-ph:database-bold text-32px text-zinc-500" />
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
                  >
                    <td style={{ padding: '12px 16px', fontWeight: 600 }}>
                      <Link
                        to="/collections/$collectionName"
                        params={{ collectionName: col.name }}
                        style={{
                          color: 'var(--jb-accent-blue)',
                          textDecoration: 'none',
                          display: 'flex',
                          alignItems: 'center',
                          gap: 8,
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
                      <span
                        style={{
                          fontSize: 11,
                          padding: '3px 8px',
                          borderRadius: 4,
                          background:
                            col.type === 'auth' ? 'rgba(135,82,163,0.15)' : 'rgba(53,116,240,0.15)',
                          color:
                            col.type === 'auth'
                              ? 'var(--jb-accent-purple)'
                              : 'var(--jb-accent-blue)',
                          fontWeight: 600,
                          display: 'inline-flex',
                          alignItems: 'center',
                          gap: 4,
                        }}
                      >
                        <span
                          className={
                            col.type === 'auth'
                              ? 'i-ph:shield-bold text-11px'
                              : 'i-ph:table-bold text-11px'
                          }
                        />
                        <span>
                          {col.type === 'auth'
                            ? t('collections.type.auth')
                            : t('collections.type.base')}
                        </span>
                      </span>
                    </td>
                    <td style={{ padding: '12px 16px', color: 'var(--jb-text-muted)' }}>
                      {col.fields?.map((f) => `${f.name} (${f.type})`).join(', ') ||
                        t('collections.no_fields')}
                    </td>
                    <td
                      style={{ padding: '12px 16px', fontSize: 12, color: 'var(--jb-text-muted)' }}
                    >
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
                            padding: '0 8px',
                            display: 'flex',
                            alignItems: 'center',
                            gap: 4,
                          }}
                        >
                          <span className="i-ph:eye-bold text-12px" />
                          <span>{t('records.title')}</span>
                        </Button>
                        <Button
                          onClick={() => openEditModal(col)}
                          style={{
                            height: 26,
                            fontSize: 12,
                            padding: '0 8px',
                            display: 'flex',
                            alignItems: 'center',
                            gap: 4,
                          }}
                        >
                          <span className="i-ph:pencil-simple-line-bold text-12px" />
                          <span>{t('app.edit')}</span>
                        </Button>
                        <Button
                          danger
                          onClick={() => handleDelete(col.name)}
                          style={{
                            height: 26,
                            fontSize: 12,
                            padding: '0 8px',
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
        </div>
      </div>

      {/* Modal / Drawer for Create/Edit Collection */}
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
              width: 680,
              maxHeight: '90vh',
              background: 'var(--jb-panel-bg)',
              border: '1px solid var(--jb-border)',
              borderRadius: 8,
              boxShadow: 'var(--jb-shadow)',
              display: 'flex',
              flexDirection: 'column',
              overflow: 'hidden',
            }}
          >
            {/* Modal Header */}
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
                  gap: 8,
                }}
              >
                <span className="i-ph:database-bold text-blue-500 text-15px" />
                <span>
                  {editingCollection
                    ? `${t('app.edit')}: ${editingCollection.name}`
                    : t('collections.create')}
                </span>
              </h3>
              <button
                onClick={closeModal}
                style={{
                  background: 'transparent',
                  border: 'none',
                  color: 'var(--jb-text-muted)',
                  cursor: 'pointer',
                  display: 'flex',
                  padding: 4,
                }}
              >
                <span className="i-ph:x-bold text-15px" />
              </button>
            </div>

            {/* Modal Body */}
            <div
              style={{
                flex: 1,
                overflowY: 'auto',
                padding: 20,
                display: 'flex',
                flexDirection: 'column',
                gap: 16,
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
                        onClick={() => handleRemoveField(idx)}
                        style={{
                          background: 'transparent',
                          border: 'none',
                          color: '#e53935',
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

                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10 }}>
                  <div>
                    <span style={{ fontSize: 11, color: 'var(--jb-text-muted)' }}>
                      {t('collections.rule.list')}
                    </span>
                    <input
                      type="text"
                      placeholder={t('collections.rule.placeholder_public')}
                      value={listRule}
                      onChange={(e) => setListRule(e.target.value)}
                      style={{
                        width: '100%',
                        padding: '4px 8px',
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
                    <span style={{ fontSize: 11, color: 'var(--jb-text-muted)' }}>
                      {t('collections.rule.view')}
                    </span>
                    <input
                      type="text"
                      placeholder={t('collections.rule.placeholder_public')}
                      value={viewRule}
                      onChange={(e) => setViewRule(e.target.value)}
                      style={{
                        width: '100%',
                        padding: '4px 8px',
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
                    <span style={{ fontSize: 11, color: 'var(--jb-text-muted)' }}>
                      {t('collections.rule.create')}
                    </span>
                    <input
                      type="text"
                      placeholder={t('collections.rule.placeholder_admin')}
                      value={createRule}
                      onChange={(e) => setCreateRule(e.target.value)}
                      style={{
                        width: '100%',
                        padding: '4px 8px',
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
                    <span style={{ fontSize: 11, color: 'var(--jb-text-muted)' }}>
                      {t('collections.rule.update')}
                    </span>
                    <input
                      type="text"
                      placeholder={t('collections.rule.placeholder_admin')}
                      value={updateRule}
                      onChange={(e) => setUpdateRule(e.target.value)}
                      style={{
                        width: '100%',
                        padding: '4px 8px',
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
                    <span style={{ fontSize: 11, color: 'var(--jb-text-muted)' }}>
                      {t('collections.rule.delete')}
                    </span>
                    <input
                      type="text"
                      placeholder={t('collections.rule.placeholder_admin')}
                      value={deleteRule}
                      onChange={(e) => setDeleteRule(e.target.value)}
                      style={{
                        width: '100%',
                        padding: '4px 8px',
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
            </div>

            {/* Modal Footer */}
            <div
              style={{
                padding: '12px 20px',
                borderTop: '1px solid var(--jb-border)',
                background: 'var(--jb-header-bg)',
                display: 'flex',
                justifyContent: 'flex-end',
                gap: 10,
              }}
            >
              <Button onClick={closeModal} style={{ height: 30 }}>
                {t('app.cancel')}
              </Button>
              <Button
                primary
                onClick={() => saveMutation.mutate()}
                disabled={saveMutation.isPending || !name}
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
    </div>
  );
}
