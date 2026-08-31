import React, { useState } from 'react';
import { Kanban, KeyRound, Loader2, LogIn, Sparkles, UserPlus } from 'lucide-react';
import { toast } from 'sonner';
import { loginWithEmail, registerWithEmail } from '../store/auth';
import { DEMO_ACCOUNTS } from '../types/kanban';
import { Button } from '../components/ui/button';
import { Input } from '../components/ui/input';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '../components/ui/card';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '../components/ui/tabs';
import { ThemeToggle } from '../components/layout/theme-toggle';

export interface LoginPageProps {
  onSuccess: () => void;
  onGoToSetup?: () => void;
}

export function LoginPage({ onSuccess, onGoToSetup }: LoginPageProps) {
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('correct password 123');
  const [displayName, setDisplayName] = useState('');
  const [isLoading, setIsLoading] = useState(false);

  const handleDemoLogin = async (demoEmail: string) => {
    setIsLoading(true);
    try {
      const user = await loginWithEmail(demoEmail);
      toast.success(`欢迎回来，${user.displayName || user.email}！`);
      onSuccess();
    } catch (err) {
      console.error(err);
      toast.error(`登录失败: ${err instanceof Error ? err.message : '请确认账号已初始化'}`);
    } finally {
      setIsLoading(false);
    }
  };

  const handleCustomLogin = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!email.trim()) return;

    setIsLoading(true);
    try {
      const user = await loginWithEmail(email.trim(), password);
      toast.success(`欢迎回来，${user.displayName || user.email}！`);
      onSuccess();
    } catch (err) {
      toast.error(`登录失败: ${err instanceof Error ? err.message : '邮箱或密码错误'}`);
    } finally {
      setIsLoading(false);
    }
  };

  const handleCustomRegister = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!email.trim() || !displayName.trim()) return;

    setIsLoading(true);
    try {
      const user = await registerWithEmail(email.trim(), displayName.trim(), password);
      toast.success(`注册成功，已自动登录为 ${user.displayName}！`);
      onSuccess();
    } catch (err) {
      toast.error(`注册失败: ${err instanceof Error ? err.message : '邮箱可能已被注册'}`);
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="min-h-screen bg-muted/20 flex flex-col items-center justify-center p-4">
      <div className="absolute top-4 right-4">
        <ThemeToggle />
      </div>

      <div className="w-full max-w-md space-y-6">
        {/* Logo */}
        <div className="text-center space-y-2">
          <div className="inline-flex h-12 w-12 items-center justify-center rounded-2xl bg-primary text-primary-foreground shadow-md">
            <Kanban className="h-6 w-6" />
          </div>
          <h1 className="text-2xl font-bold tracking-tight text-foreground">
            登录 HertaKanban
          </h1>
          <p className="text-xs text-muted-foreground">
            纯真实 HertaBase 后端鉴权体系 (`kb_users` 集合)
          </p>
        </div>

        {/* Demo Fast Login Cards */}
        <Card className="border-primary/30 shadow-sm bg-card/60 backdrop-blur-sm">
          <CardHeader className="pb-2">
            <CardTitle className="text-xs font-semibold uppercase tracking-wider text-primary flex items-center">
              <Sparkles className="mr-1.5 h-3.5 w-3.5" />
              一键快速登录演示角色
            </CardTitle>
            <CardDescription className="text-xs">
              点击下方角色以对应权限秒级登录并体验看板协作：
            </CardDescription>
          </CardHeader>
          <CardContent className="grid grid-cols-2 gap-2 pt-1">
            {DEMO_ACCOUNTS.map((acc) => (
              <button
                key={acc.email}
                type="button"
                onClick={() => handleDemoLogin(acc.email)}
                disabled={isLoading}
                className="flex flex-col items-start rounded-xl border bg-background/80 p-2.5 text-left transition-all hover:border-primary/50 hover:bg-primary/5 hover:shadow-xs cursor-pointer disabled:opacity-50"
              >
                <div className="flex items-center space-x-1.5 w-full">
                  <div className={`h-2 w-2 rounded-full ${acc.color}`} />
                  <span className="font-semibold text-xs text-foreground truncate">
                    {acc.name.split(' ')[0]}
                  </span>
                </div>
                <span className="text-[10px] text-muted-foreground mt-1 line-clamp-1">
                  {acc.description}
                </span>
              </button>
            ))}
          </CardContent>
        </Card>

        {/* Custom Auth Tabs */}
        <Card>
          <Tabs defaultValue="login" className="w-full">
            <div className="px-6 pt-4">
              <TabsList className="grid w-full grid-cols-2">
                <TabsTrigger value="login" className="text-xs">
                  <KeyRound className="mr-1.5 h-3.5 w-3.5" /> 账号登录
                </TabsTrigger>
                <TabsTrigger value="register" className="text-xs">
                  <UserPlus className="mr-1.5 h-3.5 w-3.5" /> 注册新账号
                </TabsTrigger>
              </TabsList>
            </div>

            {/* Login Tab */}
            <TabsContent value="login">
              <form onSubmit={handleCustomLogin}>
                <CardContent className="space-y-3 pt-4">
                  <div className="space-y-1">
                    <label className="text-xs font-semibold text-foreground">邮箱地址</label>
                    <Input
                      type="email"
                      placeholder="name@example.com"
                      value={email}
                      onChange={(e) => setEmail(e.target.value)}
                      className="h-8 text-xs"
                      required
                    />
                  </div>
                  <div className="space-y-1">
                    <label className="text-xs font-semibold text-foreground">登录密码</label>
                    <Input
                      type="password"
                      placeholder="******"
                      value={password}
                      onChange={(e) => setPassword(e.target.value)}
                      className="h-8 text-xs"
                      required
                    />
                  </div>
                </CardContent>
                <div className="p-6 pt-0">
                  <Button type="submit" className="w-full h-8 text-xs" disabled={isLoading}>
                    {isLoading ? <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" /> : <LogIn className="mr-1.5 h-3.5 w-3.5" />}
                    登录
                  </Button>
                </div>
              </form>
            </TabsContent>

            {/* Register Tab */}
            <TabsContent value="register">
              <form onSubmit={handleCustomRegister}>
                <CardContent className="space-y-3 pt-4">
                  <div className="space-y-1">
                    <label className="text-xs font-semibold text-foreground">姓名 / 昵称</label>
                    <Input
                      placeholder="例如：张工"
                      value={displayName}
                      onChange={(e) => setDisplayName(e.target.value)}
                      className="h-8 text-xs"
                      required
                    />
                  </div>
                  <div className="space-y-1">
                    <label className="text-xs font-semibold text-foreground">邮箱地址</label>
                    <Input
                      type="email"
                      placeholder="new-dev@example.com"
                      value={email}
                      onChange={(e) => setEmail(e.target.value)}
                      className="h-8 text-xs"
                      required
                    />
                  </div>
                  <div className="space-y-1">
                    <label className="text-xs font-semibold text-foreground">设置密码</label>
                    <Input
                      type="password"
                      value={password}
                      onChange={(e) => setPassword(e.target.value)}
                      className="h-8 text-xs"
                      required
                    />
                  </div>
                </CardContent>
                <div className="p-6 pt-0">
                  <Button type="submit" className="w-full h-8 text-xs" disabled={isLoading}>
                    {isLoading ? <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" /> : <UserPlus className="mr-1.5 h-3.5 w-3.5" />}
                    立即注册并登录
                  </Button>
                </div>
              </form>
            </TabsContent>
          </Tabs>
        </Card>

        {onGoToSetup && (
          <div className="text-center">
            <button
              type="button"
              onClick={onGoToSetup}
              className="text-xs text-muted-foreground hover:text-foreground underline cursor-pointer"
            >
              遇到 Schema 未就绪？点击前往初始化向导
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
