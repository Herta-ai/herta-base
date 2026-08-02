/** Default theme settings */
export const themeSettings: App.Theme.ThemeSetting = {
  themeScheme: 'auto',
  grayscale: false,
  colorWeakness: false,
  recommendColor: false,
  themeColor: '#646cff',
  themeRadius: 6,
  otherColor: {
    info: '#2080f0',
    success: '#52c41a',
    warning: '#faad14',
    error: '#f5222d',
  },
  isInfoFollowPrimary: true,
  page: {
    animate: true,
    animateMode: 'fade-slide',
  },
  watermark: {
    visible: false,
    text: 'SoybeanAdmin',
    enableUserName: false,
    enableTime: false,
    timeFormat: 'YYYY-MM-DD HH:mm',
  },
  tokens: {
    light: {
      colors: {
        'container': 'rgb(255, 255, 255)',
        'layout': 'rgb(247, 250, 252)',
        'inverted': 'rgb(0, 20, 40)',
        'base-text': 'rgb(31, 31, 31)',
      },
      boxShadow: {
        header: '0 1px 2px rgb(0, 21, 41, 0.08)',
        sider: '2px 0 8px 0 rgb(29, 35, 41, 0.05)',
        tab: '0 1px 2px rgb(0, 21, 41, 0.08)',
      },
    },
    dark: {
      colors: {
        'container': 'rgb(28, 28, 28)',
        'layout': 'rgb(18, 18, 18)',
        'base-text': 'rgb(224, 224, 224)',
      },
    },
  },
}
