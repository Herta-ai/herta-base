import { useNavigate } from '@tanstack/react-router';
import React, { useState, useEffect, useMemo, useRef } from 'react';

import type { CollectionModel } from '../../lib/api';
import { useI18n } from '../../lib/i18n';
import { appStore } from '../../store/app';
import { clearAuthSession } from '../../store/auth';

export interface CommandPaletteProps {
  open: boolean;
  onClose: () => void;
  collections?: CollectionModel[];
}

interface CommandItem {
  id: string;
  title: string;
  category: string;
  icon: string;
  badge?: string;
  action: () => void;
}

export function CommandPalette({ open, onClose, collections = [] }: CommandPaletteProps) {
  const navigate = useNavigate();
  const { t, lang, setLang } = useI18n();
  const [query, setQuery] = useState('');
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      setQuery('');
      setSelectedIndex(0);
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [open]);

  // Build command list
  const commands: CommandItem[] = useMemo(() => {
    const list: CommandItem[] = [
      // Navigation
      {
        id: 'nav-collections',
        title: t('nav.collections'),
        category: t('nav.workspace'),
        icon: 'i-ph:database-bold text-sky-400',
        badge: '/collections',
        action: () => {
          navigate({ to: '/collections' });
          onClose();
        },
      },
      {
        id: 'nav-logs',
        title: t('nav.logs'),
        category: t('nav.workspace'),
        icon: 'i-ph:terminal-window-bold text-emerald-400',
        badge: '/logs',
        action: () => {
          navigate({ to: '/logs' });
          onClose();
        },
      },
      {
        id: 'nav-web',
        title: t('nav.web_hosting'),
        category: t('nav.workspace'),
        icon: 'i-ph:globe-bold text-blue-400',
        badge: '/web',
        action: () => {
          navigate({ to: '/web' });
          onClose();
        },
      },
      {
        id: 'nav-settings',
        title: t('nav.settings'),
        category: t('nav.workspace'),
        icon: 'i-ph:gear-six-bold text-purple-400',
        badge: '/settings',
        action: () => {
          navigate({ to: '/settings' });
          onClose();
        },
      },
      {
        id: 'nav-swagger',
        title: t('nav.swagger'),
        category: 'External',
        icon: 'i-ph:book-open-text-bold text-amber-400',
        badge: '/swagger-ui',
        action: () => {
          window.open('/swagger-ui', '_blank');
          onClose();
        },
      },

      // Actions & Preferences
      {
        id: 'action-theme',
        title: t('app.theme.dark') + ' / ' + t('app.theme.light'),
        category: 'Preferences',
        icon: 'i-ph:sun-dim-bold text-amber-400',
        action: () => {
          appStore.setState((s) => ({ ...s, dark: !s.dark }));
          onClose();
        },
      },
      {
        id: 'action-lang',
        title: lang === 'zh' ? 'Switch to English' : '切换为简体中文',
        category: 'Preferences',
        icon: 'i-ph:translate-bold text-sky-400',
        action: () => {
          setLang(lang === 'zh' ? 'en' : 'zh');
          onClose();
        },
      },
      {
        id: 'action-logout',
        title: t('auth.logout'),
        category: 'Account',
        icon: 'i-ph:sign-out-bold text-red-400',
        action: () => {
          if (window.confirm(t('auth.logout.confirm'))) {
            clearAuthSession();
            navigate({ to: '/login' });
          }
          onClose();
        },
      },
    ];

    // Add Collections
    collections.forEach((col) => {
      list.push({
        id: `col-${col.name}`,
        title: `${col.name} (${col.type === 'auth' ? t('collections.type.auth') : t('collections.type.base')})`,
        category: t('nav.database'),
        icon:
          col.type === 'auth'
            ? 'i-ph:user-circle-bold text-amber-400'
            : 'i-ph:table-bold text-sky-400',
        badge: `${col.fields?.length || 0} fields`,
        action: () => {
          navigate({ to: '/collections/$collectionName', params: { collectionName: col.name } });
          onClose();
        },
      });
    });

    return list;
  }, [t, lang, collections, navigate, onClose, setLang]);

  // Filter commands
  const filteredCommands = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return commands;
    return commands.filter(
      (cmd) =>
        cmd.title.toLowerCase().includes(q) ||
        cmd.category.toLowerCase().includes(q) ||
        (cmd.badge && cmd.badge.toLowerCase().includes(q)),
    );
  }, [commands, query]);

  useEffect(() => {
    setSelectedIndex(0);
  }, [filteredCommands]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setSelectedIndex((prev) => (prev + 1) % Math.max(1, filteredCommands.length));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setSelectedIndex(
        (prev) => (prev - 1 + filteredCommands.length) % Math.max(1, filteredCommands.length),
      );
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (filteredCommands[selectedIndex]) {
        filteredCommands[selectedIndex].action();
      }
    } else if (e.key === 'Escape') {
      onClose();
    }
  };

  if (!open) return null;

  return (
    <div
      style={{
        position: 'fixed',
        inset: 0,
        backgroundColor: 'rgba(0, 0, 0, 0.65)',
        backdropFilter: 'blur(3px)',
        display: 'flex',
        alignItems: 'flex-start',
        justifyContent: 'center',
        paddingTop: '12vh',
        zIndex: 999999,
      }}
      onClick={onClose}
    >
      <div
        style={{
          width: 580,
          maxWidth: '92vw',
          backgroundColor: 'var(--jb-panel-bg)',
          borderRadius: 8,
          border: '1px solid var(--jb-border)',
          boxShadow: '0 24px 60px rgba(0, 0, 0, 0.55)',
          overflow: 'hidden',
          display: 'flex',
          flexDirection: 'column',
        }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Search Input Box */}
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 10,
            padding: '12px 16px',
            borderBottom: '1px solid var(--jb-border)',
            background: 'var(--jb-header-bg)',
          }}
        >
          <span className="i-ph:magnifying-glass-bold text-16px text-sky-400" />
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={t('app.search') + ' (Search Everywhere, Collections, Actions...)'}
            style={{
              flex: 1,
              border: 'none',
              background: 'transparent',
              color: 'var(--jb-text-heading)',
              fontSize: 14,
              outline: 'none',
            }}
          />
          <span
            style={{
              fontSize: 11,
              color: 'var(--jb-text-muted)',
              background: 'var(--jb-border)',
              padding: '2px 6px',
              borderRadius: 4,
            }}
          >
            ESC
          </span>
        </div>

        {/* Command Results List */}
        <div
          style={{
            maxHeight: 340,
            overflowY: 'auto',
            padding: '6px 0',
          }}
        >
          {filteredCommands.length === 0 ? (
            <div
              style={{
                padding: '24px 16px',
                textAlign: 'center',
                color: 'var(--jb-text-muted)',
                fontSize: 13,
              }}
            >
              未找到匹配的结果
            </div>
          ) : (
            filteredCommands.map((cmd, idx) => {
              const isSelected = idx === selectedIndex;
              return (
                <div
                  key={cmd.id}
                  onClick={() => cmd.action()}
                  onMouseEnter={() => setSelectedIndex(idx)}
                  style={{
                    padding: '8px 16px',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'space-between',
                    gap: 12,
                    cursor: 'pointer',
                    backgroundColor: isSelected ? 'var(--jb-active-item)' : 'transparent',
                    color: isSelected ? 'var(--jb-accent-blue)' : 'var(--jb-text)',
                  }}
                >
                  <div style={{ display: 'flex', alignItems: 'center', gap: 10, minWidth: 0 }}>
                    <span className={`${cmd.icon} text-16px shrink-0`} />
                    <span
                      style={{
                        fontSize: 13,
                        fontWeight: isSelected ? 600 : 400,
                        overflow: 'hidden',
                        textOverflow: 'ellipsis',
                        whiteSpace: 'nowrap',
                      }}
                    >
                      {cmd.title}
                    </span>
                  </div>
                  <div style={{ display: 'flex', alignItems: 'center', gap: 6, flexShrink: 0 }}>
                    {cmd.badge && (
                      <span
                        style={{
                          fontSize: 11,
                          color: 'var(--jb-text-muted)',
                          background: 'var(--jb-border)',
                          padding: '1px 6px',
                          borderRadius: 4,
                        }}
                      >
                        {cmd.badge}
                      </span>
                    )}
                    <span style={{ fontSize: 10, color: 'var(--jb-text-muted)' }}>
                      {cmd.category}
                    </span>
                  </div>
                </div>
              );
            })
          )}
        </div>

        {/* Footer info */}
        <div
          style={{
            padding: '8px 16px',
            borderTop: '1px solid var(--jb-border)',
            background: 'var(--jb-header-bg)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            fontSize: 11,
            color: 'var(--jb-text-muted)',
          }}
        >
          <div style={{ display: 'flex', gap: 12 }}>
            <span>↑↓ 导航</span>
            <span>↵ 确认选择</span>
            <span>ESC 关闭</span>
          </div>
          <span>Shift Shift / Ctrl+K</span>
        </div>
      </div>
    </div>
  );
}
