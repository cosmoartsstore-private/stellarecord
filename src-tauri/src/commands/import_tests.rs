#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_import_completion_reports_partial_success() {
        let summary = analyze::DiffImportSummary {
            total_count: 3,
            failed_filenames: vec!["output_log_failed.txt".to_string()],
        };

        assert_eq!(
            format_diff_import_completion(&summary),
            (
                "2件完了、1件失敗（成功分は保存済み）: output_log_failed.txt".to_string(),
                "2/3".to_string(),
            )
        );
    }

    #[test]
    fn diff_import_completion_reports_full_success() {
        let summary = analyze::DiffImportSummary {
            total_count: 3,
            failed_filenames: Vec::new(),
        };

        assert_eq!(
            format_diff_import_completion(&summary),
            (
                "Data 内 zst アーカイブからの取り込みが完了しました。".to_string(),
                "100%".to_string(),
            )
        );
    }
}
