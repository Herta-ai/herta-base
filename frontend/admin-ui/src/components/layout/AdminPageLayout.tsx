import React from 'react';

export interface BreadcrumbItem {
  label: React.ReactNode;
  icon?: string;
  href?: string;
}

export interface AdminPageLayoutProps {
  tabTitle: React.ReactNode;
  tabIcon?: string;
  tabBadge?: React.ReactNode;
  breadcrumbs?: BreadcrumbItem[];
  actions?: React.ReactNode;
  banner?: React.ReactNode;
  children: React.ReactNode;
  padding?: number | string;
  gap?: number | string;
  style?: React.CSSProperties;
}

export function AdminPageLayout({
  tabTitle,
  tabIcon,
  tabBadge,
  breadcrumbs = [],
  actions,
  banner,
  children,
  padding = 20,
  gap = 20,
  style,
}: AdminPageLayoutProps) {
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
      {/* 1. Editor Tabs Bar */}
      <div className="jb-editor-tabs" style={{ justifyContent: 'space-between', paddingRight: 10 }}>
        <div style={{ display: 'flex', alignItems: 'center' }}>
          <div className="jb-editor-tab active">
            {tabIcon && <span className={`${tabIcon} text-13px`} />}
            <span>{tabTitle}</span>
            {tabBadge !== undefined && tabBadge !== null && (
              <span
                style={{
                  fontSize: 10,
                  color: 'var(--jb-text-muted)',
                  background: 'var(--jb-border)',
                  padding: '1px 6px',
                  borderRadius: 10,
                  fontWeight: 600,
                }}
              >
                {tabBadge}
              </span>
            )}
          </div>
        </div>

        {/* Action Toolbar */}
        {actions && <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>{actions}</div>}
      </div>

      {/* 2. Breadcrumbs Bar */}
      <div className="jb-breadcrumbs">
        <span>HertaBase</span>
        <span className="jb-breadcrumb-sep">›</span>
        {breadcrumbs.map((crumb, idx) => {
          const isLast = idx === breadcrumbs.length - 1;
          return (
            <React.Fragment key={idx}>
              <span
                style={{
                  color: isLast ? 'var(--jb-accent-blue)' : 'inherit',
                  fontWeight: isLast ? 500 : 400,
                  display: 'inline-flex',
                  alignItems: 'center',
                  gap: 4,
                }}
              >
                {crumb.icon && <span className={`${crumb.icon} text-12px`} />}
                <span>{crumb.label}</span>
              </span>
              {!isLast && <span className="jb-breadcrumb-sep">›</span>}
            </React.Fragment>
          );
        })}
      </div>

      {/* 3. Main Content Container */}
      <div
        style={{
          flex: 1,
          overflowY: 'auto',
          overflowX: 'hidden',
          padding,
          display: 'flex',
          flexDirection: 'column',
          gap,
          boxSizing: 'border-box',
          ...style,
        }}
      >
        {banner}
        {children}
      </div>
    </div>
  );
}
