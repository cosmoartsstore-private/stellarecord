import { appDisplayName } from '../shared/constants';
import { getMsg } from '../messages/getMsg';
import { useCallback, useEffect, useRef, useState } from 'react';
import '../App.css';
import '../App.light.css';
import '../App.dark.css';
import '../App.midnight.css';
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
import { launchExternalApp, openFolder, registerApp, unregisterApp } from '../features/registry/services/registryService';
import type { AppCard, LauncherViewMode } from '../features/registry/models/types';
import { useRegistryState } from '../features/registry/viewmodels/useRegistryState';
import { RegistrySection } from '../features/registry/views/RegistrySection';
import { RegisterAppModal } from '../features/registry/views/RegisterAppModal';
import { readInitialTheme, saveTheme, type ThemeMode } from '../features/settings/models/theme';
import { useSettingsState } from '../features/settings/viewmodels/useSettingsState';
import { useStartupImportSettings } from '../features/settings/viewmodels/useStartupImportSettings';
import { SettingsControls } from '../features/settings/views/SettingsControls';
import { SettingsModal } from '../features/settings/views/SettingsModal';
import { StartupImportConsentDialog } from '../features/settings/views/StartupImportConsentDialog';
import { StellaIcon, stellaIconNames } from '../shared/components/Icons';
import logoLightSrc from '../assets/logo-light.png';
import logoDarkSrc from '../assets/logo-dark.png';
import { useToasts } from '../shared/hooks/useToasts';
import { CreditButton } from './CreditModal';
import { addErrorToast } from '../shared/lib/errors';

type SectionId = 'registry' | 'analyze' | 'database';

function App() {
  const [themeMode, setThemeMode] = useState<ThemeMode>(readInitialTheme);
  const [activeSection, setActiveSection] = useState<SectionId>('registry');
  const [launcherViewMode, setLauncherViewMode] = useState<LauncherViewMode>('list');

  const [isRegisterModalOpen, setIsRegisterModalOpen] = useState(false);
  const [isSettingsModalOpen, setIsSettingsModalOpen] = useState(false);
  const startupImportInitialCheckCompletedRef = useRef(false);
  const startupImportSettingsErrorShownRef = useRef(false);

  const { toasts, addToast } = useToasts();
  const { registryApps, isReloading, reloadRegistry, refreshRegistry } = useRegistryState();
  const { archiveLimitDraft, isStartupEnabledDraft, setArchiveLimitDraft, toggleStartup, saveArchiveLimit } = useSettingsState();
  const { startupImportSettings, startupImportSettingsError, isStartupImportSettingsLoading, isStartupImportPreferenceSaving, saveStartupImportPreference } = useStartupImportSettings();
  const { analyzeRunning: isAnalyzeRunning, analyzeProgress, analyzeStatus, storageStatus, pollStorage, setAnalyzeRunning, handleCancelSync } = useAnalyzeState(addToast);
  const { dbTables, currentTable, tableData, isDbLoading, currentPage, totalPages, pageSize, loadTableData, goToPage, sortState, toggleSort, loadDatabaseCatalog } = useDatabaseState(addToast);
  const { archiveFiles, logViewerData, isLogViewerLoading, isLogViewerLoaded, externalFiles, openEnhancedSync, openLogViewerSelection, executeEnhancedSync, openSelectedLogViewer, closeLogViewer, runStartupImport, selectExternalLogFiles, clearExternalLogFiles } = useArchiveState();

  const { selectedFiles: batchSelectedFiles, clearSelection: clearBatchSelection, handleFileAction: handleBatchFileAction, handleSelectAll: handleBatchSelectAll } = useArchiveSelection(archiveFiles.map((f) => f.name));

  const modals = useAppModals({ addToast, archiveFiles, openEnhancedSync, executeEnhancedSync, openLogViewerSelection, openSelectedLogViewer, closeLogViewer, setAnalyzeRunning, batchSelectedFiles, clearBatchSelection, selectExternalLogFiles, clearExternalLogFiles, externalFiles, });

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
      addToast(getMsg('App.applicationStarted', { name: app.name }));
    } catch (error) {
      addErrorToast(addToast, getMsg('Operations.launchApplication'), getMsg('App.applicationStartFailed', { name: app.name }), error);
    }
  };

  /** アプリの実行ファイルが格納されたフォルダを開く */
  const handleOpenFolder = async (app: AppCard) => {
    try {
      const dir = app.path.substring(0, app.path.lastIndexOf('\\'));
      await openFolder(dir);
    } catch (error) {
      addErrorToast(addToast, getMsg('Operations.openApplicationFolder'), getMsg('App.folderOpenFailed'), error);
    }
  };

  /** サードパーティアプリを登録する */
  const handleRegisterApp = async (path: string, name: string, description: string) => {
    try {
      await registerApp(path, name, description);
      addToast(getMsg('App.applicationRegistered', { name: name }));
      setIsRegisterModalOpen(false);
      await refreshRegistry();
    } catch (error) {
      addErrorToast(addToast, getMsg('Operations.registerApplication'), getMsg('App.applicationRegistrationFailed', { name: name }), error);
    }
  };

  /** サードパーティアプリの登録を解除する */
  const handleUnregisterApp = async (app: AppCard) => {
    try {
      await unregisterApp(app.path);
      addToast(getMsg('App.applicationUnregistered', { name: app.name }));
      await refreshRegistry();
    } catch (error) {
      addErrorToast(addToast, getMsg('Operations.unregisterApplication'), getMsg('App.applicationUnregistrationFailed', { name: app.name }), error);
    }
  };

  /** 自動起動のON/OFFを切り替える */
  const handleToggleStartup = async () => {
    try {
      const shouldEnable = !isStartupEnabledDraft;
      await toggleStartup();
      addToast(shouldEnable ? getMsg('App.startupEnabled') : getMsg('App.startupDisabled'));
    } catch (error) {
      addErrorToast(addToast, getMsg('Operations.saveStartupSetting'), getMsg('App.settingsSaveFailed'), error);
    }
  };

  /** 警告ラインの設定値を保存する */
  const handleSaveArchiveLimit = async () => {
    try {
      await saveArchiveLimit();
      addToast(getMsg('App.settingsSaved'));
      void pollStorage();
    } catch (error) {
      addErrorToast(addToast, getMsg('Operations.saveArchiveCapacity'), getMsg('App.settingsSaveFailed'), error);
    }
  };

  /** アプリ起動時に未圧縮ログを自動取り込みする */
  const handleStartupImport = useCallback(async () => {
    try {
      await runStartupImport();
    } catch (error) {
      addErrorToast(addToast, getMsg('Operations.startStartupImport'), getMsg('App.startupImportFailed'), error);
    }
  }, [addToast, runStartupImport]);

  /** 起動時ログ取り込みの設定を保存し、成功したかを呼び出し元へ返す。 */
  const persistStartupImportPreference = async (enabled: boolean) => {
    try {
      await saveStartupImportPreference(enabled);
      return true;
    } catch (error) {
      addErrorToast(addToast, getMsg('Operations.saveStartupImportSetting'), getMsg('App.startupImportSettingSaveFailed'), error);
      return false;
    }
  };

  /** 初回確認で許可された場合は、設定保存後に今回分のログ取り込みを開始する。 */
  const handleStartupImportConsent = async (enabled: boolean) => {
    const isSaved = await persistStartupImportPreference(enabled);
    if (isSaved && enabled) await handleStartupImport();
  };

  /** 設定画面から次回以降の起動時ログ取り込みを切り替える。 */
  const handleToggleStartupImport = async () => {
    const shouldEnable = !(startupImportSettings?.enabled ?? false);
    const isSaved = await persistStartupImportPreference(shouldEnable);
    if (isSaved) {
      addToast(
        shouldEnable
          ? getMsg('App.startupImportEnabled')
          : getMsg('App.startupImportDisabled'),
      );
    }
  };

  /**
   * 設定画面で選択されたテーマへ切り替える
   * 切替時にCSSトランジションを一時無効化し、中間状態のちらつきを防ぐ
   */
  const handleThemeChange = (nextThemeMode: ThemeMode) => {
    if (nextThemeMode === themeMode) return;
    document.documentElement.classList.add('disable-transitions');
    setThemeMode(nextThemeMode);
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        document.documentElement.classList.remove('disable-transitions');
      });
    });
  };

  // テーマ変更時にブラウザストレージへ永続化
  useEffect(() => {
    saveTheme(themeMode);
  }, [themeMode]);

  // 初回取得時点で許可済みの場合だけ、アプリ起動ごとに1回取り込みを実行
  useEffect(() => {
    if (!startupImportSettings || startupImportInitialCheckCompletedRef.current) return;
    startupImportInitialCheckCompletedRef.current = true;
    if (startupImportSettings.preference_set && startupImportSettings.enabled) {
      void handleStartupImport();
    }
  }, [handleStartupImport, startupImportSettings]);

  useEffect(() => {
    if (!startupImportSettingsError || startupImportSettingsErrorShownRef.current) return;
    startupImportSettingsErrorShownRef.current = true;
    addErrorToast(addToast, getMsg('Operations.loadDataLocations'), getMsg('App.dataLocationsLoadFailed'), startupImportSettingsError);
  }, [addToast, startupImportSettingsError]);

  const navItems: { section: SectionId; label: string }[] = [{ section: 'registry', label: getMsg('common.launcher') }, { section: 'analyze', label: getMsg('common.analyze') }, { section: 'database', label: getMsg('common.database') }];

  const navIcons = { registry: stellaIconNames.rocket, analyze: stellaIconNames.chartBar, database: stellaIconNames.tableGrid } as const;

  /** アクティブセクションに対応するコンポーネントを描画する */
  const renderSection = () => {
    switch (activeSection) {
      case 'registry':
        return (
          <RegistrySection registryApps={registryApps} launcherViewMode={launcherViewMode} isReloading={isReloading} onSetLauncherViewMode={setLauncherViewMode} onLaunchApp={(app) => {
              void handleLaunch(app);
            }} onOpenFolder={(app) => {
              void handleOpenFolder(app);
            }} onUnregisterApp={(app) => {
              void handleUnregisterApp(app);
            }} onRegisterApp={() => {
              setIsRegisterModalOpen(true);
            }} onReload={() => {
              void reloadRegistry();
            }} />
        );
      case 'analyze':
        return (
          <AnalyzeSection storageStatus={storageStatus} isAnalyzeRunning={isAnalyzeRunning} analyzeProgress={analyzeProgress} analyzeStatus={analyzeStatus} onRefreshStorage={() => {
              void pollStorage();
            }} onOpenEnhancedSync={() => {
              void modals.handleOpenEnhancedSync();
            }} onOpenLogViewer={() => {
              void modals.handleOpenLogViewer();
            }} onCancelSync={() => {
              void handleCancelSync();
            }} onOpenCleanup={() => {
              void modals.handleOpenCleanup();
            }} />
        );
      case 'database':
        return (
          <DatabaseSection dbTables={dbTables} currentTable={currentTable} tableData={tableData} isDbLoading={isDbLoading} currentPage={currentPage} totalPages={totalPages} pageSize={pageSize} sortState={sortState} onSelectTable={(tableName) => {
              void loadTableData(tableName);
            }} onGoToPage={(page) => {
              void goToPage(page);
            }} onToggleSort={(columnName) => {
              void toggleSort(columnName);
            }} />
        );
    }
  };

  return (
    <div className={`stella-record-root ${styles.mainWrapper} ${styles.root} ${themeMode}-theme`}>
      <nav className={styles.topNavigation} aria-label={getMsg('App.mainNavigation')}>
        <div className={styles.navBrand}>
          <img src={themeMode === 'light' ? logoLightSrc : logoDarkSrc} alt={appDisplayName} className={
              styles[
                themeMode === 'midnight'
                  ? 'navLogoMidnight'
                  : themeMode === 'dark'
                    ? 'navLogoDark'
                    : 'navLogoLight'
              ]
            } />
        </div>

        <div className={styles.pillNav}>
          {navItems.map(({ section, label }) => (
            <button key={section} type="button" className={`${styles.pillBtn} ${activeSection === section ? styles.pillBtnActive : ''}`} aria-current={activeSection === section ? 'page' : undefined} onClick={() => {
                void handleNavSelect(section);
              }}>
              <StellaIcon name={navIcons[section]} />
              {label}
            </button>
          ))}
        </div>

        <button type="button" className={styles.navSettingsButton} onClick={() => { setIsSettingsModalOpen(true); }}><StellaIcon name={stellaIconNames.settings} />{getMsg('common.settings')}</button>
      </nav>

      <main className={styles.contentArea}>{renderSection()}</main>

      {modals.isArchiveSelectorVisible && (
        <ArchiveSelectorModal archiveFiles={archiveFiles} selectedFiles={batchSelectedFiles} onClose={modals.closeArchiveSelector} onSelectAll={handleBatchSelectAll} onFileAction={handleBatchFileAction} onConfirm={() => {
            void modals.handleConfirmImport();
          }} />
      )}

      {modals.isLogViewerModalVisible && logViewerData && (
        <LogViewerModal logViewerData={logViewerData} archiveFiles={archiveFiles} externalFiles={externalFiles} isLoading={isLogViewerLoading} isLoaded={isLogViewerLoaded} onNavigateToFile={modals.handleViewerNavigateToFile} onPickExternalFiles={() => {
            void modals.handleSelectExternalFiles();
          }} onClearExternalFiles={() => {
            void modals.handleClearExternalFiles();
          }} onClose={modals.closeLogViewerModal} />
      )}

      {modals.isCleanupModalOpen && (
        <PolarisCleanupModal logs={modals.deletableLogs} onClose={modals.closeCleanupModal} onConfirm={(fileNames) => {
            void modals.handleConfirmCleanup(fileNames);
          }} />
      )}

      {isRegisterModalOpen && (
        <RegisterAppModal onClose={() => {
            setIsRegisterModalOpen(false);
          }} onConfirm={(path, name, description) => {
            void handleRegisterApp(path, name, description);
          }} />
      )}

      {isSettingsModalOpen && (
        <SettingsModal onClose={() => {
            setIsSettingsModalOpen(false);
          }}>
          <SettingsControls themeMode={themeMode} archiveLimitDraft={archiveLimitDraft} isStartupEnabledDraft={isStartupEnabledDraft} isStartupImportEnabled={startupImportSettings?.enabled ?? false} isStartupImportLoading={ isStartupImportSettingsLoading || isStartupImportPreferenceSaving } logArchivePath={startupImportSettings?.log_archive_path ?? ''} databasePath={startupImportSettings?.database_path ?? ''} isDataLocationLoading={isStartupImportSettingsLoading} onThemeModeChange={handleThemeChange} onArchiveLimitDraftChange={setArchiveLimitDraft} onSaveArchiveLimit={() => {
              void handleSaveArchiveLimit();
            }} onToggleStartup={() => {
              void handleToggleStartup();
            }} onToggleStartupImport={() => {
              void handleToggleStartupImport();
            }} />
        </SettingsModal>
      )}

      {startupImportSettings && !startupImportSettings.preference_set && (
        <StartupImportConsentDialog logArchivePath={startupImportSettings.log_archive_path} databasePath={startupImportSettings.database_path} isSaving={isStartupImportPreferenceSaving} onAllow={() => {
            void handleStartupImportConsent(true);
          }} onDecline={() => {
            void handleStartupImportConsent(false);
          }} />
      )}

      <CreditButton />
      <ToastContainer toasts={toasts} />
    </div>
  );
}

export default App;
