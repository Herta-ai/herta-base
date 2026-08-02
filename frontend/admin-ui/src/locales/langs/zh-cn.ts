const local: I18n.Schema = {
  common: {
    backToHome: '返回首页',
  },
  system: {
    title: 'Vue3 基础模板',
    updateTitle: '系统版本更新通知',
    updateContent: '检测到系统有新版本发布，是否立即刷新页面？',
    updateConfirm: '立即刷新',
    updateCancel: '稍后再说',
  },
  route: {
    'login': '登录',
    'not-found': '页面不存在',
    'home': '首页',
  },
  page: {
    home: {
      title: '首页',
    },
    login: {
      form: {
        username: {
          required: '请输入用户名',
          invalid: '用户名最小 6 个字符',
          placeholder: '请输入用户名',
        },
        password: {
          required: '请输入密码',
          invalid: '密码最小 6 个字符',
          placeholder: '请输入密码',
        },
      },
      confirm: '登录',
      loginSuccess: '登录成功',
      welcomeBack: '欢迎回来 {username}！',
    },
  },
  datatable: {
    itemCount: '共 {total} 条',
  },
}

export default local
