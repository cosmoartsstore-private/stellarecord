/**
 * StellaRecordの一元管理SVGアイコンシステム
 *
 * Material Design系の24x24 SVGパスを静的マップに格納。
 * 単一の <StellaIcon name="..." /> コンポーネントで描画し、
 * 個別SVGファイルのインポートを不要にしてバンドルサイズを予測可能にする
 */
import type { SVGProps } from 'react';

/** アイコン定義: SVGパス文字列とオプションのviewBox/スタイルオーバーライド */
interface StellaIconDefinition {
  viewBox?: string;
  className?: string;
  style?: SVGProps<SVGSVGElement>['style'];
  path: string;
}

/** コンパイル時に安全なアイコン名定数 — 文字列リテラルの代わりにこちらを使用する */
export const stellaIconNames = {
  alert: 'alert',
  bell: 'bell',
  chartBar: 'chartBar',
  eclipse: 'eclipse',
  folder: 'folder',
  moon: 'moon',
  refresh: 'refresh',
  rocket: 'rocket',
  sparkle: 'sparkle',
  sun: 'sun',
  tableGrid: 'tableGrid',
  list: 'list',
  grid: 'grid',
  info: 'info',
  plus: 'plus',
  trash: 'trash',
  power: 'power',
} as const;

/** stellaIconNames マップから導出される有効なアイコン名のユニオン型 */
type StellaIconName = (typeof stellaIconNames)[keyof typeof stellaIconNames];

/** アイコンサイズ用の共有CSSクラスプリセット */
const stellaIconClassNames = {
  base: 'icon-svg',
  small: 'icon-svg',
} as const;

/** 名前をキーとするアイコン定義の完全レジストリ */
const stellaIconMap: Record<StellaIconName, StellaIconDefinition> = {
  alert: {
    path: 'M13,14H11V10H13M13,18H11V16H13M1,21H23L12,2L1,21Z',
  },
  bell: {
    style: { width: '22px', height: '22px' },
    path: 'M21,19V20H3V19L5,17V11C5,7.9 7.03,5.17 10,4.29C10,4.19 10,4.1 10,4A2,2 0 0,1 12,2A2,2 0 0,1 14,4C14,4.1 14,4.19 14,4.29C16.97,5.17 19,7.9 19,11V17L21,19M14,21A2,2 0 0,1 12,23A2,2 0 0,1 10,21',
  },
  chartBar: {
    path: 'M22,21H2V3H4V19H6V10H10V19H12V6H16V19H18V14H22V21Z',
  },
  eclipse: {
    path: 'M12,2A10,10 0 0,0 2,12A10,10 0 0,0 12,22A10,10 0 0,0 22,12A10,10 0 0,0 12,2M12,4A8,8 0 0,1 20,12A8,8 0 0,1 12,20A8,8 0 0,1 4,12A8,8 0 0,1 12,4M12,6A6,6 0 0,0 6,12A6,6 0 0,0 12,18A6,6 0 0,0 18,12A6,6 0 0,0 12,6Z',
  },
  folder: {
    path: 'M10,4H4C2.89,4 2,4.89 2,6V18A2,2 0 0,0 4,20H20A2,2 0 0,0 22,18V8C22,6.89 21.1,6 20,6H12L10,4Z',
  },
  moon: {
    path: 'M17.75,4.09L15.22,6.03L16.13,9.09L13.5,7.28L10.87,9.09L11.78,6.03L9.25,4.09L12.44,4L13.5,1L14.56,4L17.75,4.09M21.25,11L19.61,12.25L20.2,14.23L18.5,13.06L16.8,14.23L17.39,12.25L15.75,11L17.81,10.95L18.5,9L19.19,10.95L21.25,11M18.97,15.95C19.8,15.87 20.69,17.05 20.16,17.8C19.84,18.25 19.5,18.67 19.08,19.07C15.17,23 8.84,23 4.94,19.07C1.03,15.17 1.03,8.83 4.94,4.93C5.34,4.53 5.76,4.17 6.21,3.85C6.96,3.32 8.14,4.21 8.06,5.04C7.79,7.9 8.75,10.87 10.95,13.06C13.14,15.26 16.1,16.22 18.97,15.95M17.33,17.97C14.5,17.81 11.7,16.64 9.53,14.5C7.36,12.31 6.2,9.5 6.04,6.68C3.23,9.82 3.23,14.4 6.04,17.55C8.83,20.7 13.4,20.7 16.19,17.55L17.33,17.97Z',
  },
  rocket: {
    path: 'M16,20H20V16H16M16,14H20V10H16M10,8H14V4H10M16,8H20V4H16M10,14H14V10H10M4,14H8V10H4M4,20H8V16H4M10,20H14V16H10M4,8H8V4H4V8Z',
  },
  sun: {
    path: 'M12,18A6,6 0 0,1 6,12A6,6 0 0,1 12,6A6,6 0 0,1 18,12A6,6 0 0,1 12,18M20,11H23V13H20V11M1,11H4V13H1V11M13,1V4H11V1H13M13,20V23H11V20H13M4.92,3.5L6.34,4.92L4.92,6.34L3.5,4.92L4.92,3.5M17.66,16.66L19.07,18.07L17.66,19.49L16.24,18.07L17.66,16.66M19.07,5.93L17.66,7.34L16.24,5.93L17.66,4.5L19.07,5.93M6.34,19.07L4.92,17.66L6.34,16.24L7.76,17.66L6.34,19.07Z',
  },
  tableGrid: {
    path: 'M5,4H19A2,2 0 0,1 21,6V18A2,2 0 0,1 19,20H5A2,2 0 0,0 3,18V6A2,2 0 0,0 5,4M5,8V12H11V8H5M13,8V12H19V8H13M5,14V18H11V14H5M13,14V18H19V14H13Z',
  },
  refresh: {
    path: 'M17.65,6.35C16.2,4.9 14.21,4 12,4A8,8 0 0,0 4,12A8,8 0 0,0 12,20C15.73,20 18.84,17.45 19.73,14H17.65C16.83,16.33 14.61,18 12,18A6,6 0 0,1 6,12A6,6 0 0,1 12,6C13.66,6 15.14,6.69 16.22,7.78L13,11H20V4L17.65,6.35Z',
  },
  sparkle: {
    path: 'M12,2L14.47,7.29L20.24,8.13L16.06,12.2L17.05,17.94L12,15.29L6.95,17.94L7.94,12.2L3.76,8.13L9.53,7.29L12,2Z',
  },
  list: {
    path: 'M4 5a1 1 0 0 0 0 2h1a1 1 0 0 0 0-2H4zm5 0a1 1 0 0 0 0 2h11a1 1 0 0 0 0-2H9zM4 11a1 1 0 1 0 0 2h1a1 1 0 1 0 0-2H4zm5 0a1 1 0 1 0 0 2h11a1 1 0 1 0 0-2H9zM4 17a1 1 0 1 0 0 2h1a1 1 0 1 0 0-2H4zm5 0a1 1 0 1 0 0 2h11a1 1 0 1 0 0-2H9z',
  },
  grid: {
    path: 'M3 3a1 1 0 0 1 1-1h6a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V3zm11 0a1 1 0 0 1 1-1h6a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1h-6a1 1 0 0 1-1-1V3zM3 14a1 1 0 0 1 1-1h6a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1v-6zm11 0a1 1 0 0 1 1-1h6a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1h-6a1 1 0 0 1-1-1v-6z',
  },
  info: {
    path: 'M13,9H11V7H13M13,17H11V11H13M12,2A10,10 0 0,0 2,12A10,10 0 0,0 12,22A10,10 0 0,0 22,12A10,10 0 0,0 12,2Z',
  },
  plus: {
    path: 'M19,13H13V19H11V13H5V11H11V5H13V11H19V13Z',
  },
  trash: {
    path: 'M19,4H15.5L14.5,3H9.5L8.5,4H5V6H19M6,19A2,2 0 0,0 8,21H16A2,2 0 0,0 18,19V7H6V19Z',
  },
  power: {
    path: 'M13,3H11V13H13V3M17.83,5.17L16.41,6.59C18.05,7.91 19,9.85 19,12A7,7 0 0,1 12,19A7,7 0 0,1 5,12C5,9.85 5.95,7.91 7.59,6.59L6.17,5.17C4.23,6.82 3,9.26 3,12A9,9 0 0,0 12,21A9,9 0 0,0 21,12C21,9.26 19.77,6.82 17.83,5.17Z',
  },
};

interface StellaIconProps extends Omit<SVGProps<SVGSVGElement>, 'name'> {
  name: StellaIconName;
}

/** 組み込みアイコンマップからインラインSVGを描画する（標準SVG propsでオーバーライド可） */
export function StellaIcon({ name, className, ...props }: StellaIconProps) {
  const icon = stellaIconMap[name];

  return (
    <svg
      viewBox={icon.viewBox ?? '0 0 24 24'}
      className={className ?? icon.className ?? stellaIconClassNames.base}
      style={icon.style}
      {...props}
    >
      <path d={icon.path} />
    </svg>
  );
}
