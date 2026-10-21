#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn ensure_parent_dir_creates_missing_parent() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir
            .path()
            .join("nested")
            .join("sub")
            .join("stellarecord.db");
        assert!(!db_path.parent().unwrap().exists());

        ensure_parent_dir(&db_path).unwrap();
        assert!(db_path.parent().unwrap().exists());
    }

    #[test]
    fn ensure_parent_dir_noop_when_parent_exists() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("stellarecord.db");
        // 親（tempdir 自体）は既に存在する
        ensure_parent_dir(&db_path).unwrap();
        assert!(dir.path().exists());
    }

    #[test]
    fn ensure_parent_dir_ok_for_rootless_path() {
        // 親を持たない相対パスでもエラーにならない
        ensure_parent_dir(std::path::Path::new("file.db")).unwrap();
    }

    include!("native_command_tests.rs");
    include!("native_cross_app_tests.rs");
    include!("native_single_instance_tests.rs");
    include!("native_sibling_registry_tests.rs");
    include!("native_headless_outcome_tests.rs");
    include!("native_ipc_tests.rs");
    include!("native_startup_tests.rs");
}
