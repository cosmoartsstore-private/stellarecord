import { useCallback, useEffect, useState } from 'react';
import '../App.css';
import '../App.light.css';
import '../App.dark.css';
import '../App.midnight.css';
import type { SectionId } from './section';
import styles from './App.module.css';
import { useAppModals } from './useAppModals';
import { ToastContainer } from './ToastContainer';
import { useArchiveState } from '../features/archive/viewmodels/useArchiveState';
import { useArchiveSelection } from '../features/archive/viewmodels/useArchiveSelection';
import { ArchiveSelectorModal } from '../features/archive/views/ArchiveSelectorModal';
import { LogViewerModal } from '../features/archive/views/LogViewerModal';
import { AnalyzeSection } from '../features/analyze/views/AnalyzeSection';
import { PolarisCleanupModal } from '../features/analyze/views/PolarisCleanupModal';
import { useAnalyzeState } from '../features/analyze/viewmodels/useAnalyzeState';
import { useDatabaseState } from '../features/database/viewmodels/useDatabaseState';
import { DatabaseSection } from '../features/database/views/DatabaseSection';
import {
  launchExternalApp,
  openFolder,
  registerApp,
  unregisterApp,
} from '../features/registry/services/registryService';
import type { AppCard } from '../features/registry/models/types';
import { useRegistryState } from '../features/registry/viewmodels/useRegistryState';
import { RegistrySection } from '../features/registry/views/RegistrySection';
import { RegisterAppModal } from '../features/registry/views/RegisterAppModal';
import { readInitialTheme, saveTheme } from '../features/settings/models/theme';
import type { ThemeMode } from '../features/settings/models/types';
import { useSettingsState } from '../features/settings/viewmodels/useSettingsState';
import { SettingsControls } from '../features/settings/views/SettingsControls';
import { StellaIcon, stellaIconNames } from '../shared/components/Icons';
import logoLightSrc from '../assets/logo-light.png';
import logoDarkSrc from '../assets/logo-dark.png';
import { useToasts } from '../shared/hooks/useToasts';
import { CreditButton } from './CreditModal';
import { addErrorToast } from '../shared/lib/errors';

/** ランチャーグリッドの表示モード */
type LauncherViewMode = 'list' | 'card';

const themeCycle: ThemeMode[] = ['light', 'dark', 'midnight'];

function App() {
  const [themeMode, setThemeMode] = useState<ThemeMode>(readInitialTheme);
  const [activeSection, setActiveSection] = useState<SectionId>('registry');
  const [launcherViewMode, setLauncherViewMode] = useState<LauncherViewMode>('list');

  const [isRegisterModalOpen, setIsRegisterModalOpen] = useState(false);

  const { toasts, addToast } = useToasts();
  const { registryApps, isReloading, reloadRegistry, refreshRegistry } = useRegistryState();
  const {
    archiveLimitDraft,
    isStartupEnabledDraft,
    setArchiveLimitDraft,
    toggleStartup,
    saveArchiveLimit,
  } = useSettingsState();
  const {
    analyzeRunning: isAnalyzeRunning,
    analyzeProgress,
    analyzeStatus,
    storageStatus,
    pollStorage,
    setAnalyzeRunning,
    handleCancelSync,
  } = useAnalyzeState(addToast);
  const {
    dbTables,
    currentTable,
    tableData,
    isDbLoading,
    currentPage,
    totalPages,
    pageSize,
    loadTableData,
    goToPage,
    sortState,
    toggleSort,
    loadDatabaseCatalog,
  } = useDatabaseState(addToast);
  const {
    archiveFiles,
    logViewerData,
    isLogViewerLoading,
    isLogViewerLoaded,
    externalFiles,
    openEnhancedSync,
    openLogViewerSelection,
    executeEnhancedSync,
    openSelectedLogViewer,
    closeLogViewer,
    runStartupImport,
    selectExternalLogFiles,
    clearExternalLogFiles,
  } = useArchiveState();

  const {
    selectedFiles: batchSelectedFiles,
    clearSelection: clearBatchSelection,
    handleFileAction: handleBatchFileAction,
    handleSelectAll: handleBatchSelectAll,
  } = useArchiveSelection(archiveFiles.map((f) => f.name));

  const modals = useAppModals({
    addToast,
    archiveFiles,
    openEnhancedSync,
    executeEnhancedSync,
    openLogViewerSelection,
    openSelectedLogViewer,
    closeLogViewer,
    setAnalyzeRunning,
    batchSelectedFiles,
    clearBatchSelection,
    selectExternalLogFiles,
    clearExternalLogFiles,
    externalFiles,
  });

  /** タブ選択時にセクションを切り替える（DB選択時はカタログを自動取得） */
  const handleNavSelect = async (section: SectionId) => {
    setActiveSection(section);
    if (section === 'database') {
      await loadDatabaseCatalog();
    }
  };

  /** 登録済みアプリの実行ファイルを起動する */
  const handleLaunch = async (app: AppCard) => {
    try {
      await launchExternalApp(app.path);
      addToast(`${app.name} を起動しました`);
    } catch (error) {
      addErrorToast(addToast, '登録済みアプリの起動', `${app.name} の起動に失敗しました`, error);
    }
  };

  /** アプリの実行ファイルが格納されたフォルダを開く */
  const handleOpenFolder = async (app: AppCard) => {
    try {
      const dir = app.path.substring(0, app.path.lastIndexOf('\\'));
      await openFolder(dir);
    } catch (error) {
      addErrorToast(
        addToast,
        '登録済みアプリのフォルダを開く',
        'フォルダを開けませんでした',
        error,
      );
    }
  };

  /** サードパーティアプリを登録する */
  const handleRegisterApp = async (path: string, name: string, description: string) => {
    try {
      await registerApp(path, name, description);
      addToast(`${name} を登録しました`);
      setIsRegisterModalOpen(false);
      await refreshRegistry();
    } catch (error) {
      addErrorToast(addToast, 'サードパーティアプリ登録', `${name} の登録に失敗しました`, error);
    }
  };

  /** サードパーティアプリの登録を解除する */
  const handleUnregisterApp = async (app: AppCard) => {
    try {
      await unregisterApp(app.path);
      addToast(`${app.name} の登録を解除しました`);
      await refreshRegistry();
    } catch (error) {
      addErrorToast(
        addToast,
        'サードパーティアプリ登録解除',
        `${app.name} の登録解除に失敗しました`,
        error,
      );
    }
  };

  /** 自動起動のON/OFFを切り替える */
  const handleToggleStartup = async () => {
    try {
      const shouldEnable = !isStartupEnabledDraft;
      await toggleStartup();
      addToast(shouldEnable ? '自動起動を有効にしました' : '自動起動を無効にしました');
    } catch (error) {
      addErrorToast(addToast, '自動起動設定保存', '設定保存に失敗しました', error);
    }
  };

  /** 警告ラインの設定値を保存する */
  const handleSaveArchiveLimit = async () => {
    try {
      await saveArchiveLimit();
      addToast('設定を保存しました');
      void pollStorage();
    } catch (error) {
      addErrorToast(addToast, 'アーカイブ容量設定保存', '設定保存に失敗しました', error);
    }
  };

  /** アプリ起動時に未圧縮ログを自動取り込みする */
  const handleStartupImport = useCallback(async () => {
    try {
      await runStartupImport();
    } catch (error) {
      addErrorToast(addToast, '起動時取り込み開始', '起動時取り込みを開始できませんでした', error);
    }
  }, [addToast, runStartupImport]);

  /**
   * テーマを light → dark → midnight の順で切り替える
   * 切替時にCSSトランジションを一時無効化し、中間状態のちらつきを防ぐ
   */
  const handleThemeToggle = () => {
    document.documentElement.classList.add('disable-transitions');
    setThemeMode((prev) => {
      const idx = themeCycle.indexOf(prev);
      return themeCycle[(idx + 1) % themeCycle.length] ?? 'light';
    });
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        document.documentElement.classList.remove('disable-transitions');
      });
    });
  };

  const themeIcon =
    themeMode === 'light'
      ? stellaIconNames.sun
      : themeMode === 'dark'
        ? stellaIconNames.moon
        : stellaIconNames.eclipse;

  // テーマ変更時にブラウザストレージへ永続化
  useEffect(() => {
    saveTheme(themeMode);
  }, [themeMode]);

  // マウント時に起動取り込みを実行
  useEffect(() => {
    void handleStartupImport();
  }, [handleStartupImport]);

  const navItems: { section: SectionId; label: string }[] = [
    { section: 'registry', label: 'ランチャー' },
    { section: 'analyze', label: '解析' },
    { section: 'database', label: 'DB' },
  ];

  const navIcons = {
    registry: stellaIconNames.rocket,
    analyze: stellaIconNames.chartBar,
    database: stellaIconNames.tableGrid,
  } as const;

  /** アクティブセクションに対応するコンポーネントを描画する */
  const renderSection = () => {
    switch (activeSection) {
      case 'registry':
        return (
          <RegistrySection
            registryApps={registryApps}
            launcherViewMode={launcherViewMode}
            isReloading={isReloading}
            onSetLauncherViewMode={setLauncherViewMode}
            onLaunchApp={(app) => {
              void handleLaunch(app);
            }}
            onOpenFolder={(app) => {
              void handleOpenFolder(app);
            }}
            onUnregisterApp={(app) => {
              void handleUnregisterApp(app);
            }}
            onRegisterApp={() => {
              setIsRegisterModalOpen(true);
            }}
            onReload={() => {
              void reloadRegistry();
            }}
          />
        );
      case 'analyze':
        return (
          <AnalyzeSection
            storageStatus={storageStatus}
            isAnalyzeRunning={isAnalyzeRunning}
            analyzeProgress={analyzeProgress}
            analyzeStatus={analyzeStatus}
            settingsControls={
              <SettingsControls
                archiveLimitDraft={archiveLimitDraft}
                isStartupEnabledDraft={isStartupEnabledDraft}
                onArchiveLimitDraftChange={setArchiveLimitDraft}
                onSaveArchiveLimit={() => {
                  void handleSaveArchiveLimit();
                }}
                onToggleStartup={() => {
                  void handleToggleStartup();
                }}
              />
            }
            onRefreshStorage={() => {
              void pollStorage();
            }}
            onOpenEnhancedSync={() => {
              void modals.handleOpenEnhancedSync();
            }}
            onOpenLogViewer={() => {
              void modals.handleOpenLogViewer();
            }}
            onCancelSync={() => {
              void handleCancelSync();
            }}
            onOpenCleanup={() => {
              void modals.handleOpenCleanup();
            }}
          />
        );
      case 'database':
        return (
          <DatabaseSection
            dbTables={dbTables}
            currentTable={currentTable}
            tableData={tableData}
            isDbLoading={isDbLoading}
            currentPage={currentPage}
            totalPages={totalPages}
            pageSize={pageSize}
            sortState={sortState}
            onSelectTable={(tableName) => {
              void loadTableData(tableName);
            }}
            onGoToPage={(page) => {
              void goToPage(page);
            }}
            onToggleSort={(columnName) => {
              void toggleSort(columnName);
            }}
          />
        );
    }
  };

  return (
    <div className={`stella-record-root ${styles.mainWrapper} ${styles.root} ${themeMode}-theme`}>
      <nav className={styles.topNavigation}>
        <div className={styles.navBrand}>
          <img
            src={themeMode === 'light' ? logoLightSrc : logoDarkSrc}
            alt="STELLA RECORD"
            className={
              styles[
                themeMode === 'midnight'
                  ? 'navLogoMidnight'
                  : themeMode === 'dark'
                    ? 'navLogoDark'
                    : 'navLogoLight'
              ]
            }
          />
        </div>

        <div className={styles.pillNav}>
          {navItems.map(({ section, label }) => (
            <button
              key={section}
              className={`${styles.pillBtn} ${activeSection === section ? styles.pillBtnActive : ''}`}
              onClick={() => {
                void handleNavSelect(section);
              }}
            >
              <StellaIcon name={navIcons[section]} />
              {label}
            </button>
          ))}
        </div>

        <button
          className={styles.navSettingsButton}
          onClick={handleThemeToggle}
          aria-label="テーマ切替"
        >
          <StellaIcon name={themeIcon} />
        </button>
      </nav>

      <main className={styles.contentArea}>{renderSection()}</main>

      {modals.isArchiveSelectorVisible && (
        <ArchiveSelectorModal
          archiveFiles={archiveFiles}
          selectedFiles={batchSelectedFiles}
          onClose={modals.closeArchiveSelector}
          onSelectAll={handleBatchSelectAll}
          onFileAction={handleBatchFileAction}
          onConfirm={() => {
            void modals.handleConfirmImport();
          }}
        />
      )}

      {modals.isLogViewerModalVisible && logViewerData && (
        <LogViewerModal
          logViewerData={logViewerData}
          archiveFiles={archiveFiles}
          externalFiles={externalFiles}
          isLoading={isLogViewerLoading}
          isLoaded={isLogViewerLoaded}
          onNavigateToFile={modals.handleViewerNavigateToFile}
          onPickExternalFiles={() => {
            void modals.handleSelectExternalFiles();
          }}
          onClearExternalFiles={() => {
            void modals.handleClearExternalFiles();
          }}
          onClose={modals.closeLogViewerModal}
        />
      )}

      {modals.isCleanupModalOpen && (
        <PolarisCleanupModal
          logs={modals.deletableLogs}
          onClose={modals.closeCleanupModal}
          onConfirm={(fileNames) => {
            void modals.handleConfirmCleanup(fileNames);
          }}
        />
      )}

      {isRegisterModalOpen && (
        <RegisterAppModal
          onClose={() => {
            setIsRegisterModalOpen(false);
          }}
          onConfirm={(path, name, description) => {
            void handleRegisterApp(path, name, description);
          }}
        />
      )}

      <CreditButton />
      <ToastContainer toasts={toasts} />
    </div>
  );
}

export default App;
