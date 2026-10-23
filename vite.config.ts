import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  plugins: [react()],
  test: {
    // Windowsで画面テストのworker初期化が停止するため、実行方式と同時数を固定する。
    pool: 'threads',
    maxWorkers: 2,
    // coverageを伴う画面操作がWindowsで既定の5秒を超えるため、操作列の実行時間を確保する。
    testTimeout: 15000,
    coverage: {
      include: ['src/**/*.{ts,tsx}'],
      exclude: ['src/**/*.d.ts', 'src/test/**'],
      // 利用者の表示・操作・結果を合否とし、被覆率は確認不足を探す資料として出力する。
    },
  },
});
