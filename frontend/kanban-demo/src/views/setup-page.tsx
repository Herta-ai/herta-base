import React, { useEffect, useState } from 'react';
import {
  AlertCircle,
  CheckCircle2,
  Database,
  ExternalLink,
  Kanban,
  Loader2,
  RefreshCw,
  Rocket,
  ShieldAlert,
  Terminal,
} from 'lucide-react';
import { toast } from 'sonner';
import { checkDatabaseInitialized, type CheckResult } from '../lib/init-checker';
import { initializeAndSeedKanban } from '../lib/seed-service';
import { loginWithEmail } from '../store/auth';
import { setActiveWorkspaceId } from '../store/workspace';
import { Button } from '../components/ui/button';
import { Input } from '../components/ui/input';
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from '../components/ui/card';
import { ThemeToggle } from '../components/layout/theme-toggle';

export interface SetupPageProps {
  onInitialized: () => void;
}

export function SetupPage({ onInitialized }: SetupPageProps) {
  const [checking, setChecking] = useState(true);
  const [checkResult, setCheckResult] = useState<CheckResult | null>(null);

  const [adminEmail, setAdminEmail] = useState('admin@example.com');
  const [adminPassword, setAdminPassword] = useState('correct horse battery staple');

  const [isSeeding, setIsSeeding] = useState(false);
  const [progressMsg, setProgressMsg] = useState('');
  const [progressPercent, setProgressPercent] = useState(0);

  const runCheck = async () => {
    setChecking(true);
    try {
      const res = await checkDatabaseInitialized();
      setCheckResult(res);
      if (res.status === 'ready') {
        toast.success('检测到数据库集合已就绪！');
      }
    } finally {
      setChecking(false);
    }
  };

  useEffect(() => {
    void runCheck();
  }, []);

  const handleInitAndSeed = async (e: React.FormEvent) => {
    e.preventDefault();
    if (isSeeding) return;

    setIsSeeding(true);
    setProgressPercent(5);
    setProgressMsg('正在连接 HertaBase 服务端...');

    try {
      const result = await initializeAndSeedKanban(
        adminEmail,
        adminPassword,
        (step, percent) => {
          setProgressMsg(step);
          setProgressPercent(percent);
        },
      );

      // Auto login as owner
      await loginWithEmail('owner@example.com');
      setActiveWorkspaceId(result.workspaceId);

      toast.success('看板系统初始化完成！已为您自动登录 Owner 账号');
      onInitialized();
    } catch (err) {
      console.error(err);
      toast.error(`初始化失败: ${err instanceof Error ? err.message : '请确认管理员凭证正确'}`);
    } finally {
      setIsSeeding(false);
    }
  };

  return (
    <div className="min-h-screen bg-muted/20 flex flex-col items-center justify-center p-4">
      <div className="absolute top-4 right-4">
        <ThemeToggle />
      </div>

      <div className="w-full max-w-xl space-y-6">
        {/* Header Logo */}
        <div className="text-center space-y-2">
          <div className="inline-flex h-12 w-12 items-center justify-center rounded-2xl bg-primary text-primary-foreground shadow-md">
            <Kanban className="h-6 w-6" />
          </div>
          <h1 className="text-2xl font-bold tracking-tight text-foreground">
            HertaKanban 数据库初始化向导
          </h1>
          <p className="text-xs text-muted-foreground max-w-md mx-auto">
            检测到当前 HertaBase 后端尚未创建 Kanban 业务集合。请通过下方一键建表向导完成自动配置。
          </p>
        </div>

        {/* Server Status Check Card */}
        <Card>
          <CardHeader className="pb-3">
            <div className="flex items-center justify-between">
              <CardTitle className="text-sm flex items-center">
                <Database className="mr-2 h-4 w-4 text-primary" />
                HertaBase 后端连通性与 Schema 状态
              </CardTitle>
              <Button
                size="sm"
                variant="ghost"
                className="h-7 text-xs"
                onClick={runCheck}
                disabled={checking || isSeeding}
              >
                {checking ? <Loader2 className="h-3 w-3 animate-spin mr-1" /> : <RefreshCw className="h-3 w-3 mr-1" />}
                重新检测
              </Button>
            </div>
          </CardHeader>
          <CardContent className="space-y-3">
            {checking ? (
              <div className="flex items-center space-x-2 text-xs text-muted-foreground py-2">
                <Loader2 className="h-4 w-4 animate-spin text-primary" />
                <span>正在探测后端 OpenAPI 规范及 Collections 状态...</span>
              </div>
            ) : checkResult?.status === 'ready' ? (
              <div className="rounded-lg bg-emerald-500/10 border border-emerald-500/20 p-3 text-xs text-emerald-600 dark:text-emerald-400 space-y-1">
                <div className="flex items-center font-semibold">
                  <CheckCircle2 className="mr-1.5 h-4 w-4" /> 数据库 Schema 已就绪！
                </div>
                <p>已检测到全部 4 个看板核心集合 (kb_users, kb_workspaces, kb_tasks, kb_comments)。</p>
                <div className="pt-2">
                  <Button size="sm" onClick={onInitialized} className="h-7 text-xs">
                    直接进入看板系统
                  </Button>
                </div>
              </div>
            ) : checkResult?.status === 'unreachable' ? (
              <div className="rounded-lg bg-destructive/10 border border-destructive/20 p-3 text-xs text-destructive space-y-2">
                <div className="flex items-center font-semibold">
                  <ShieldAlert className="mr-1.5 h-4 w-4" /> 无法连接到 HertaBase 服务端
                </div>
                <p>{checkResult.message}</p>
                <div className="bg-background/80 rounded p-2 text-[11px] font-mono border text-foreground flex items-center space-x-2">
                  <Terminal className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
                  <span>cargo run -p herta_server -- serve --db-engine memory</span>
                </div>
              </div>
            ) : (
              <div className="rounded-lg bg-amber-500/10 border border-amber-500/20 p-3 text-xs text-amber-600 dark:text-amber-400 space-y-1">
                <div className="flex items-center font-semibold">
                  <AlertCircle className="mr-1.5 h-4 w-4" /> 待初始化看板集合
                </div>
                <p>{checkResult?.message || '尚未检测到看板业务集合。'}</p>
              </div>
            )}

            <div className="text-[11px] text-muted-foreground space-y-1 border-t pt-2">
              <p className="font-semibold text-foreground">本次初始化将自动创建以下内容：</p>
              <ul className="list-disc pl-4 space-y-0.5">
                <li><code>kb_users</code> (Auth 集合，包含 Owner、Assignee、Member、Outsider 4个演示账号)</li>
                <li><code>kb_workspaces</code> (Base 集合，团队研发与外部隔离 2个工作区)</li>
                <li><code>kb_tasks</code> (Base 集合，含泳道状态、优先级、多附件、图关系展开规则)</li>
                <li><code>kb_comments</code> (Base 集合，任务评论流)</li>
              </ul>
            </div>
          </CardContent>
        </Card>

        {/* Init Form */}
        <Card>
          <form onSubmit={handleInitAndSeed}>
            <CardHeader className="pb-3">
              <CardTitle className="text-sm">管理员授权与一键初始化</CardTitle>
              <CardDescription className="text-xs">
                输入 HertaBase 超级管理员凭据执行建表（默认填充开发环境默认管理员凭据）。
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-3">
              <div className="space-y-1">
                <label className="text-xs font-semibold text-foreground">管理员邮箱</label>
                <Input
                  type="email"
                  value={adminEmail}
                  onChange={(e) => setAdminEmail(e.target.value)}
                  className="h-8 text-xs"
                  required
                />
              </div>
              <div className="space-y-1">
                <label className="text-xs font-semibold text-foreground">管理员密码</label>
                <Input
                  type="password"
                  value={adminPassword}
                  onChange={(e) => setAdminPassword(e.target.value)}
                  className="h-8 text-xs"
                  required
                />
              </div>

              {/* Progress indicator */}
              {isSeeding && (
                <div className="space-y-1.5 pt-2">
                  <div className="flex justify-between text-xs text-muted-foreground font-medium">
                    <span>{progressMsg}</span>
                    <span>{progressPercent}%</span>
                  </div>
                  <div className="h-2 w-full rounded-full bg-muted overflow-hidden">
                    <div
                      className="h-full bg-primary transition-all duration-300 rounded-full"
                      style={{ width: `${progressPercent}%` }}
                    />
                  </div>
                </div>
              )}
            </CardContent>
            <CardFooter className="flex justify-between pt-2">
              <span className="text-[11px] text-muted-foreground flex items-center">
                纯 100% 真实 API 对接 · 无 Mock
              </span>
              <Button type="submit" disabled={isSeeding || checkResult?.status === 'unreachable'}>
                {isSeeding ? (
                  <>
                    <Loader2 className="mr-1.5 h-4 w-4 animate-spin" />
                    正在初始化...
                  </>
                ) : (
                  <>
                    <Rocket className="mr-1.5 h-4 w-4" />
                    一键初始化看板与演示数据
                  </>
                )}
              </Button>
            </CardFooter>
          </form>
        </Card>
      </div>
    </div>
  );
}
