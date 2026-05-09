import tseslint from 'typescript-eslint';
import jsxA11y from 'eslint-plugin-jsx-a11y';
import react from 'eslint-plugin-react';
import reactHooks from 'eslint-plugin-react-hooks';
import unicorn from 'eslint-plugin-unicorn';

export default tseslint.config(
  tseslint.configs.strictTypeChecked,
  tseslint.configs.stylisticTypeChecked,

  {
    plugins: {
      'jsx-a11y': jsxA11y,
      react,
      'react-hooks': reactHooks,
      unicorn,
    },
    rules: {
      'react-hooks/rules-of-hooks': 'error',
      'react-hooks/exhaustive-deps': 'warn',
      'jsx-a11y/no-static-element-interactions': 'error',
      'jsx-a11y/click-events-have-key-events': 'error',
      'jsx-a11y/interactive-supports-focus': 'error',
      'unicorn/prefer-at': 'error',
      'unicorn/prefer-string-replace-all': 'error',
      'unicorn/no-array-for-each': 'error',
      'unicorn/prefer-array-flat': 'error',
      'unicorn/prefer-module': 'error',
      'unicorn/no-array-reduce': 'off',
      '@typescript-eslint/naming-convention': [
        'error',
        {
          selector: 'typeLike',
          format: ['PascalCase'],
        },
        {
          selector: 'variable',
          format: ['camelCase', 'PascalCase'],
        },
        {
          selector: 'variable',
          types: ['boolean'],
          format: ['StrictPascalCase'],
          prefix: ['is', 'has', 'can', 'should'],
        },
        {
          selector: 'function',
          format: ['camelCase', 'PascalCase'],
        },
      ],
      'no-console': 'warn',
      'no-debugger': 'error',
      eqeqeq: ['error', 'always'],
      'prefer-const': 'error',
      '@typescript-eslint/no-explicit-any': 'error',
      '@typescript-eslint/no-non-null-assertion': 'error',
      '@typescript-eslint/no-unused-vars': 'error',
      'no-restricted-imports': [
        'error',
        {
          patterns: [
            {
              group: ['../features/*', '../../features/*', '../../../features/*'],
              message: 'shared から features を参照しないでください。',
            },
            {
              group: ['../app/*', '../../app/*', '../../../app/*'],
              message: 'shared から app を参照しないでください。',
            },
          ],
        },
      ],
    },
  },

  {
    files: ['src/shared/**/*.{ts,tsx}'],
    rules: {
      'no-restricted-imports': [
        'error',
        {
          patterns: [
            {
              group: ['../features/*', '../../features/*', '../../../features/*'],
              message: 'shared から features を参照しないでください。',
            },
            {
              group: ['../app/*', '../../app/*', '../../../app/*'],
              message: 'shared から app を参照しないでください。',
            },
          ],
        },
      ],
    },
  },

  {
    files: ['src/features/**/*.{ts,tsx}'],
    rules: {
      'no-restricted-imports': [
        'error',
        {
          patterns: [
            {
              group: ['../app/*', '../../app/*', '../../../app/*'],
              message: 'features から app を参照しないでください。',
            },
            {
              group: ['../features/*', '../../features/*', '../../../features/*'],
              message: 'feature 間依存は禁止です。',
            },
          ],
        },
      ],
    },
  },

  {
    files: ['src/app/**/*.{ts,tsx}'],
    rules: {
      'no-restricted-imports': 'off',
    },
  },

  {
    languageOptions: {
      parserOptions: {
        project: ['./tsconfig.app.json', './tsconfig.node.json'],
        tsconfigRootDir: process.cwd(),
      },
    },
  },

  {
    ignores: [
      'dist/**',
      'target/**',
      'src-tauri/gen/**',
      'src-tauri/tauri.conf.json',
      'tauri.conf.json',
      'vite.config.*',
      'eslint.config.js',
    ],
  },
);
