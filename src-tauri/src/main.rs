// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(feature = "export-types")]
    {
        mejiro_streams_lib::export::export_types();
        return;
    }

    mejiro_streams_lib::run()
}
