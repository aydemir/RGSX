import js from '@eslint/js'
import pluginVue from 'eslint-plugin-vue'

export default [
  js.configs.recommended,
  ...pluginVue.configs['flat/recommended'],
  {
    files: ['src/**/*.{js,vue}'],
    languageOptions: {
      ecmaVersion: 'latest',
      sourceType: 'module',
    },
    rules: {
      'no-unused-vars': ['warn', { argsIgnorePattern: '^_', varsIgnorePattern: '^_' }],
      'no-undef': 'off',
      'no-empty': 'warn',
      'vue/multi-word-component-names': 'off',
      'vue/no-unused-vars': 'warn',
      // Format/bayi kıyaslaması — şimdilik iskelet aşamasında kapat (ileride --fix ile düzenlenir)
      'vue/max-attributes-per-line': 'off',
      'vue/html-indent': 'off',
      'vue/html-closing-bracket-newline': 'off',
      'vue/multiline-html-element-content-newline': 'off',
      'vue/singleline-html-element-content-newline': 'off',
      'vue/html-self-closing': 'off',
      'vue/attributes-order': 'off',
      'vue/first-attribute-linebreak': 'off',
    },
  },
  {
    files: ['scripts/**/*.mjs'],
    rules: {
      'no-undef': 'off',
    },
  },
  {
    ignores: ['dist/**', 'node_modules/**', 'src/i18n.strings.js'],
  },
]
