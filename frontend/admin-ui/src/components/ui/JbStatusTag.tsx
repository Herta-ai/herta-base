import React from 'react';

export type JbStatusVariant = 'success' | 'warning' | 'error' | 'info' | 'default';

export interface JbStatusTagProps {
  variant?: JbStatusVariant;
  children: React.ReactNode;
  icon?: string;
  size?: 'sm' | 'md';
  style?: React.CSSProperties;
  className?: string;
}

export function JbStatusTag({
  variant = 'default',
  children,
  icon,
  size = 'sm',
  style,
  className = '',
}: JbStatusTagProps) {
  const getColors = () => {
    switch (variant) {
      case 'success':
        return {
          bg: 'rgba(73, 156, 84, 0.12)',
          color: 'var(--jb-accent-green, #499c54)',
          border: 'rgba(73, 156, 84, 0.3)',
        };
      case 'warning':
        return {
          bg: 'rgba(245, 158, 11, 0.12)',
          color: '#f59e0b',
          border: 'rgba(245, 158, 11, 0.3)',
        };
      case 'error':
        return {
          bg: 'rgba(239, 68, 68, 0.12)',
          color: '#ef4444',
          border: 'rgba(239, 68, 68, 0.3)',
        };
      case 'info':
        return {
          bg: 'rgba(53, 116, 240, 0.12)',
          color: 'var(--jb-accent-blue, #3574f0)',
          border: 'rgba(53, 116, 240, 0.3)',
        };
      case 'default':
      default:
        return {
          bg: 'var(--jb-header-bg)',
          color: 'var(--jb-text-muted)',
          border: 'var(--jb-border)',
        };
    }
  };

  const colors = getColors();
  const isSm = size === 'sm';

  return (
    <span
      className={`jb-status-tag ${className}`}
      style={{
        display: 'inline-flex',
        alignItems: 'center',
        gap: 4,
        padding: isSm ? '1px 6px' : '3px 8px',
        borderRadius: 4,
        fontSize: isSm ? 11 : 12,
        fontWeight: 600,
        backgroundColor: colors.bg,
        color: colors.color,
        border: `1px solid ${colors.border}`,
        whiteSpace: 'nowrap',
        ...style,
      }}
    >
      {icon && <span className={`${icon} ${isSm ? 'text-11px' : 'text-12px'}`} />}
      {children}
    </span>
  );
}
