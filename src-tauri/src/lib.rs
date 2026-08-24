mod commands;
mod state;

pub mod db;
pub mod domain;

use tauri::Manager;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            let diretorio_dados = app.path().app_data_dir()?;
            let conn = db::abrir(&diretorio_dados)?;
            app.manage(AppState::new(conn));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::status_commands::get_status,
            commands::auth_commands::setup_primeiro_usuario,
            commands::auth_commands::login,
            commands::auth_commands::listar_usuarios,
            commands::auth_commands::criar_usuario,
            commands::movimento_commands::criar_movimento,
            commands::movimento_commands::listar_movimentos_do_dia,
            commands::movimento_commands::sugestoes_descricao,
            commands::fechamento_commands::fechar_dia,
            commands::fechamento_commands::buscar_fechamento_do_dia,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
