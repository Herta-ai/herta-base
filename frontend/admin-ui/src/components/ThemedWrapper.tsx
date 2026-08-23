import { useStore } from '@tanstack/react-store';
import React from 'react';

import { appStore } from '../store/app';

export interface ThemedWrapperProps {
  children: React.ReactNode;
  className?: string;
  style?: React.CSSProperties;
}

export function ThemedWrapper({ children, className = '', style }: ThemedWrapperProps) {
  const isDark = useStore(appStore, (state) => state.dark);

  return (
    <div
      className={`themed-wrapper ${isDark ? 'theme-dark ring-ui-theme-dark' : 'theme-light'} ${className}`}
      style={{ width: '100%', height: '100%', display: 'flex', flexDirection: 'column', ...style }}
    >
      {children}
    </div>
  );
}
