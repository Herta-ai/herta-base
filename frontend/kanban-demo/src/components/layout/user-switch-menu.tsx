import React from 'react';
import { useSelector } from '@tanstack/react-store';
import { Check, LogIn, LogOut, ShieldCheck, UserCheck, Users } from 'lucide-react';
import { toast } from 'sonner';
import { useQueryClient } from '@tanstack/react-query';
import { authStore, loginWithEmail, logoutUser } from '../../store/auth';
import { DEMO_ACCOUNTS } from '../../types/kanban';
import { Avatar, AvatarFallback } from '../ui/avatar';
import { Button } from '../ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '../ui/dropdown-menu';
import { getInitials, getRandomColor } from '../../lib/utils';
import { kanbanKeys } from '../../hooks/use-kanban-data';

export function UserSwitchMenu() {
  const { user, isAuthenticated } = useSelector(authStore, (s) => s);
  const queryClient = useQueryClient();

  const handleSwitch = async (email: string) => {
    try {
      const switchedUser = await loginWithEmail(email);
      queryClient.invalidateQueries({ queryKey: kanbanKeys.allWorkspaces });
      toast.success(`已切换为角色「${switchedUser.displayName}」`);
    } catch (err) {
      console.error(err);
      toast.error(`切换登录失败: ${err instanceof Error ? err.message : '请确认账号已初始化'}`);
    }
  };

  const handleLogout = async () => {
    await logoutUser();
    toast.info('已退出登录');
  };

  if (!isAuthenticated || !user) {
    return (
      <Button size="sm" variant="outline" className="h-8 text-xs" onClick={() => (window.location.href = '/login')}>
        <LogIn className="mr-1.5 h-3.5 w-3.5" />
        登录系统
      </Button>
    );
  }

  const displayName = user.displayName || user.email;

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          className="flex items-center space-x-2 rounded-full border bg-card py-1 pl-1 pr-2.5 shadow-xs hover:border-primary/50 transition-colors cursor-pointer"
        >
          <Avatar className="h-6 w-6">
            <AvatarFallback className={`${getRandomColor(displayName)} text-[10px]`}>
              {getInitials(displayName)}
            </AvatarFallback>
          </Avatar>
          <span className="text-xs font-semibold text-foreground max-w-[100px] truncate">
            {displayName}
          </span>
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-64">
        <DropdownMenuLabel className="font-normal">
          <div className="flex flex-col space-y-1">
            <p className="text-xs font-semibold leading-none">{displayName}</p>
            <p className="text-[11px] leading-none text-muted-foreground">{user.email}</p>
          </div>
        </DropdownMenuLabel>
        <DropdownMenuSeparator />

        <DropdownMenuGroup>
          <DropdownMenuLabel className="text-[11px] text-muted-foreground font-medium">
            一键切换演示角色 (测试 API Rules 权限)
          </DropdownMenuLabel>
          {DEMO_ACCOUNTS.map((acc) => {
            const isCurrent = user.email === acc.email;
            return (
              <DropdownMenuItem
                key={acc.email}
                onClick={() => !isCurrent && handleSwitch(acc.email)}
                className="flex items-start space-x-2 py-2 text-xs cursor-pointer"
              >
                <div className={`mt-0.5 h-2 w-2 rounded-full shrink-0 ${acc.color}`} />
                <div className="flex-1">
                  <div className="flex items-center justify-between font-medium">
                    <span>{acc.name}</span>
                    {isCurrent && <Check className="h-3 w-3 text-primary" />}
                  </div>
                  <p className="text-[10px] text-muted-foreground leading-tight mt-0.5">
                    {acc.description}
                  </p>
                </div>
              </DropdownMenuItem>
            );
          })}
        </DropdownMenuGroup>

        <DropdownMenuSeparator />
        <DropdownMenuItem onClick={handleLogout} className="text-xs text-destructive focus:text-destructive">
          <LogOut className="mr-2 h-3.5 w-3.5" />
          退出登录
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
