import antfu from '@antfu/eslint-config'
import sonarjs from 'eslint-plugin-sonarjs'

export default antfu(
  {
    formatters: true,
    ignores: [
      'coverage/**',
      'dist/**',
    ],
    stylistic: {
      indent: 2,
      quotes: 'single',
      semi: false,
    },
  },
  {
    name: 'hertabase/sonarjs',
    plugins: {
      sonarjs,
    },
    rules: sonarjs.configs.recommended.rules,
  },
  {
    name: 'hertabase/examples',
    files: ['docs/examples/**/*.ts'],
    rules: {
      'no-console': 'off',
    },
  },
)
