import React from 'react';
import type { RealtimeStatus } from '@hb/sdk';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '../ui/tooltip';

export interface LiveIndicatorProps {
  status: RealtimeStatus;
}

export function LiveIndicator({ status }: LiveIndicatorProps) {
  const getConfig = () => {
    switch (status) {
      case 'connected':
        return {
          dotClass: 'bg-emerald-500 animate-pulse',
          label: 'SSE 实时协同已就绪',
          badgeText: 'Live',
          badgeClass: 'bg-emerald-500/10 text-emerald-500 border-emerald-500/20',
        };
      case 'connecting':
      case 'reconnecting':
        return {
          dotClass: 'bg-amber-500 animate-ping',
          label: '正在建立 SSE 订阅连接...',
          badgeText: 'Syncing',
          badgeClass: 'bg-amber-500/10 text-amber-500 border-amber-500/20',
        };
      case 'closed':
      default:
        return {
          dotClass: 'bg-slate-400',
          label: '未连接实时流',
          badgeText: 'Offline',
          badgeClass: 'bg-slate-500/10 text-slate-500 border-slate-500/20',
        };
    }
  };

  const config = getConfig();

  return (
    <TooltipProvider>
      <Tooltip>
        <TooltipTrigger asChild>
          <div
            className={`inline-flex items-center space-x-1.5 rounded-full border px-2 py-0.5 text-xs font-medium cursor-help transition-colors ${config.badgeClass}`}
          >
            <span className={`h-1.5 w-1.5 rounded-full ${config.dotClass}`} />
            <span>{config.badgeText}</span>
          </div>
        </TooltipTrigger>
        <TooltipContent>
          <p className="text-xs">{config.label}</p>
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  );
}
