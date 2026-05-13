/** ランチャーに表示される1つのアプリケーションエントリ */
export interface AppCard {
  name: string;
  description: string;
  path: string;
  /** 登録元アプリが書き込むBase64エンコードPNGアイコン（未設定時は省略） */
  icon_data?: string;
}

/** ランチャーが扱うアプリ一覧。区別なしの平坦リスト。 */
export interface RegistryCatalog {
  apps: AppCard[];
}
