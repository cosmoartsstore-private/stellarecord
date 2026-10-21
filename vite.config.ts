import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  plugins: [react()],
  test: {
    coverage: {
      include: ['src/**/*.{ts,tsx}'],
      exclude: ['src/**/*.d.ts', 'src/test/**'],
      // 利用者の表示・操作・結果を合否とし、被覆率は確認不足を探す資料として出力する。
    },
  },
});
