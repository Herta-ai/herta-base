const local: I18n.Schema = {
  common: {
    backToHome: 'Back to Home',
  },
  system: {
    title: 'Vue3 Essentials Starter',
    updateTitle: 'System Version Update Notification',
    updateContent:
      'A new version of the system has been detected. Do you want to refresh the page immediately?',
    updateConfirm: 'Refresh immediately',
    updateCancel: 'Later',
  },
  route: {
    'login': 'Login',
    'not-found': 'Page Not Found',
    'home': 'Home',
  },
  page: {
    home: {
      title: 'Home',
    },
    login: {
      form: {
        username: {
          required: 'Please enter a username',
          invalid: 'Username must be at least 6 characters',
          placeholder: 'Please enter a username',
        },
        password: {
          required: 'Please enter a password',
          invalid: 'Password must be at least 6 characters',
          placeholder: 'Please enter a password',
        },
      },
      confirm: 'Login',
      loginSuccess: 'Login Success',
      welcomeBack: 'Welcome back {username}!',
    },
  },
  datatable: {
    itemCount: 'Total {total} items',
  },
}

export default local
