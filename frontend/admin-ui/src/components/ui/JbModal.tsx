import React, { useEffect } from 'react';

export interface JbModalProps {
  open: boolean;
  onClose: () => void;
  title?: React.ReactNode;
  icon?: string;
  children: React.ReactNode;
  footer?: React.ReactNode;
  width?: number | string;
  maxWidth?: number | string;
  closeOnEsc?: boolean;
  closeOnClickOutside?: boolean;
}

export function JbModal({
  open,
  onClose,
  title,
  icon,
  children,
  footer,
  width = 560,
  maxWidth = '92vw',
  closeOnEsc = true,
  closeOnClickOutside = true,
}: JbModalProps) {
  useEffect(() => {
    if (!open || !closeOnEsc) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [open, closeOnEsc, onClose]);

  if (!open) return null;

  return (
    <div
      style={{
        position: 'fixed',
        inset: 0,
        backgroundColor: 'rgba(0, 0, 0, 0.6)',
        backdropFilter: 'blur(2px)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 99999,
        padding: 16,
      }}
      onClick={closeOnClickOutside ? onClose : undefined}
    >
      <div
        style={{
          width,
          maxWidth,
          maxHeight: '90vh',
          backgroundColor: 'var(--jb-panel-bg)',
          borderRadius: 8,
          border: '1px solid var(--jb-border)',
          boxShadow: '0 20px 48px rgba(0, 0, 0, 0.45)',
          display: 'flex',
          flexDirection: 'column',
          overflow: 'hidden',
        }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div
          style={{
            padding: '12px 16px',
            borderBottom: '1px solid var(--jb-border)',
            background: 'var(--jb-header-bg)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            gap: 10,
          }}
        >
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, minWidth: 0 }}>
            {icon && <span className={`${icon} text-16px text-sky-400 shrink-0`} />}
            <div
              style={{
                fontSize: 13,
                fontWeight: 700,
                color: 'var(--jb-text-heading)',
                overflow: 'hidden',
                textOverflow: 'ellipsis',
                whiteSpace: 'nowrap',
              }}
            >
              {title}
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            style={{
              background: 'transparent',
              border: 'none',
              color: 'var(--jb-text-muted)',
              cursor: 'pointer',
              display: 'flex',
              padding: 4,
              borderRadius: 4,
            }}
          >
            <span className="i-ph:x-bold text-13px" />
          </button>
        </div>

        {/* Body */}
        <div
          style={{
            padding: '16px 20px',
            overflowY: 'auto',
            flex: 1,
          }}
        >
          {children}
        </div>

        {/* Footer */}
        {footer && (
          <div
            style={{
              padding: '10px 16px',
              borderTop: '1px solid var(--jb-border)',
              background: 'var(--jb-header-bg)',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'flex-end',
              gap: 8,
            }}
          >
            {footer}
          </div>
        )}
      </div>
    </div>
  );
}
