/**
 * アプリケーションのエントリーポイント — ReactツリーをTauriのWebViewにマウントする
 *
 * フォントインポートは副作用のみ。@font-faceルールをグローバルに登録し、
 * 全コンポーネントから M PLUS 1（UIテキスト）と JetBrains Mono（コード/ログ）を参照可能にする
 */
import '@fontsource-variable/m-plus-1';
import '@fontsource/jetbrains-mono/400.css';
import '@fontsource/jetbrains-mono/500.css';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import App from './app/App';

/** TauriのHTMLテンプレートに埋め込まれたDOMマウント先 */
const rootElement = document.getElementById('root');

if (rootElement === null) {
  throw new Error('root element was not found');
}

createRoot(rootElement).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
