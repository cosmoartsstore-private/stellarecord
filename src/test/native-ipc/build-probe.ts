import * as esbuild from 'esbuild';

// Browser用tsconfigへNodeのglobal timer宣言を追加せず、専用driverが使う組込みI/Oだけを型付けする。
const pathSpecifier = 'node:path';
const fsSpecifier = 'node:fs';
const urlSpecifier = 'node:url';
const processSpecifier = 'node:process';
const { default: path } = await import(pathSpecifier) as { default: { resolve(...parts: string[]): string; basename(value: string): string; dirname(value: string): string; join(...parts: string[]): string; relative(from: string, to: string): string; isAbsolute(value: string): boolean } };
const { default: fs } = await import(fsSpecifier) as { default: { statSync(value: string): { isDirectory(): boolean }; writeFileSync(file: string, data: string, encoding: string): void } };
const url = await import(urlSpecifier) as { fileURLToPath(value: string): string };
const { default: process } = await import(processSpecifier) as { default: { argv: string[] } };

// 検証用の一時領域へ、現行FrontendとTauri SDKをまとめて出力する。
const outputArgument = process.argv.at(2);
if (!outputArgument) throw new Error('専用のprobe出力先が必要です。');
const output = path.resolve(outputArgument);
if (path.basename(output) !== 'browser-probe.js' || !fs.statSync(path.dirname(output)).isDirectory()) throw new Error('専用のprobe出力先が必要です。');
const mode = process.argv.at(3) ?? 'service';
if (!['service', 'ui', 'ui-production', 'ui-picker', 'startup'].includes(mode)) throw new Error('未対応のprobeです。');
const directory = path.dirname(url.fileURLToPath(import.meta.url));
const entry = mode === 'startup' ? path.join(directory, 'startup-probe.ts') : path.join(directory, '../../app/native-ipc', mode === 'service' ? 'probe.ts' : 'ui-probe.ts');
const result = esbuild.buildSync({ entryPoints: [entry], outfile: output,
  bundle: true, platform: 'browser', format: 'iife', target: 'es2022', jsx: 'automatic', sourcemap: false, logLevel: 'warning', metafile: true,
  alias: mode === 'ui-picker' ? { '@tauri-apps/api/core': path.join(directory, 'picker-boundary.ts') } : undefined,
  define: { 'process.env.NODE_ENV': JSON.stringify(mode === 'ui-production' ? 'production' : 'development') },
  assetNames: 'probe-assets/[name]-[hash]', loader: { '.properties': 'text', '.png': 'file', '.jpg': 'file', '.woff': 'file', '.woff2': 'file' } });
const files = Object.keys(result.metafile.outputs).map(file => {
  const relative = path.relative(path.dirname(output), path.resolve(file));
  if (path.isAbsolute(relative) || relative.startsWith('..')) throw new Error('probeの出力が専用領域の外です。');
  return relative.replaceAll('\\', '/');
});
fs.writeFileSync(path.join(path.dirname(output), 'browser-probe-assets.json'), JSON.stringify(files), 'utf8');
