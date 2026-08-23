import React from 'react';

export interface JbCardProps {
  title?: React.ReactNode;
  icon?: string;
  subtitle?: React.ReactNode;
  actions?: React.ReactNode;
  footer?: React.ReactNode;
  children: React.ReactNode;
  style?: React.CSSProperties;
  bodyStyle?: React.CSSProperties;
  className?: string;
  noBodyPadding?: boolean;
}

export function JbCard({
  title,
  icon,
  subtitle,
  actions,
  footer,
  children,
  style,
  bodyStyle,
  className = '',
  noBodyPadding = false,
}: JbCardProps) {
  const hasHeader = Boolean(title || actions);

  return (
    <div className={`jb-card ${className}`} style={style}>
      {hasHeader && (
        <div className="jb-card-header">
          <div style={{ display: 'flex', alignItems: 'center', gap: 7, minWidth: 0 }}>
            {icon && <span className={`${icon} text-15px shrink-0`} />}
            <div className="jb-card-title" style={{ minWidth: 0 }}>
              {typeof title === 'string' ? <span>{title}</span> : title}
              {subtitle && (
                <span className="jb-card-subtitle">
                  {typeof subtitle === 'string' ? `· ${subtitle}` : subtitle}
                </span>
              )}
            </div>
          </div>
          {actions && (
            <div style={{ display: 'flex', alignItems: 'center', gap: 6, flexShrink: 0 }}>
              {actions}
            </div>
          )}
        </div>
      )}

      <div className={`jb-card-body ${noBodyPadding ? 'no-padding' : ''}`} style={bodyStyle}>
        {children}
      </div>

      {footer && <div className="jb-card-footer">{footer}</div>}
    </div>
  );
}
