use specta_typescript::{BigIntExportBehavior, Typescript};
use tauri_specta::{Builder, collect_commands};
use std::path::PathBuf;

use crate::commands;

/// TypeScript 型バインディングを `src/lib/bindings.ts` に出力する。
/// 実行: `cargo run --features export-types`
pub fn export_types() {
    let out_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("src")
        .join("lib")
        .join("bindings.ts");

    commands::commands_builder()
        .export(
            Typescript::default().bigint(BigIntExportBehavior::Number),
            out_path,
        )
        .expect("Failed to export typescript bindings");
}