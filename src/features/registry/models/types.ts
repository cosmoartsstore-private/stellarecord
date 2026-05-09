/** ランチャーに表示される1つのアプリケーションエントリ */
export interface AppCard {
  name: string;
  description: string;
  path: string;
  category: 'fastparty' | 'thirdparty';
  /** 登録元アプリが書き込むBase64エンコードPNGアイコン（未設定時は省略） */
  icon_data?: string;
}

/** StellaRecordのUI区分ごとにグループ化されたアプリ一覧 */
export interface RegistryCatalog {
  fastparty: AppCard[];
  thirdparty: AppCard[];
}
