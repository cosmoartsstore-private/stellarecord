/** アプリにアイコンが登録されていない場合に表示するSVGプレースホルダー */
export function LauncherFallbackIcon() {
  return (
    <svg viewBox="0 0 24 24" className="icon-svg">
      <path d="M16,9H19L14,16L9,9H12V5H16M11,2H13V4H11V2M15,19V17H17V19H15M11,19V17H13V19H11M7,19V17H9V19H7Z" />
    </svg>
  );
}
