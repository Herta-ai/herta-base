import { QueryClient } from '@tanstack/react-query';

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      refetchOnWindowFocus: false, // 离开页面再回来时是否自动请求接口
      retry: 1, // 失败重试次数
      staleTime: 5 * 60 * 1000, // 5分钟内数据认为是新鲜的，不会重复请求网络
    },
  },
});
