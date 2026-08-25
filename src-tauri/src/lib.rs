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

            if let Err(e) = db::backup::backup_automatico(&conn, &diretorio_dados) {
                log::warn!("Falha ao fazer backup automatico do banco: {e}");
            }

            // Backup externo (pendrive/HD): so acontece se `backup_externo.txt`
            // estiver configurado nesta maquina e a unidade estiver conectada
            // agora - melhor-esforco, nunca trava a abertura do app.
            if let Some(destino) = db::backup::ler_destino_externo(&diretorio_dados) {
                if let Err(e) = db::backup::backup_externo(&conn, &destino) {
                    log::warn!("Falha ao fazer backup externo do banco: {e}");
                }
            }

            app.manage(AppState::new(conn));

            // Sincronizacao oportunista com o Turso (se configurada nesta
            // maquina via turso.txt): tentativa em segundo plano, nunca
            // trava a abertura do app se nao tiver internet ou o arquivo
            // nao existir. Mesma logica de `sincronizar_agora`, mas sem
            // exigir sessao (roda antes de qualquer login).
            if let Some((url, token)) = db::sync::ler_config_turso(&diretorio_dados) {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let state = app_handle.state::<AppState>();
                    let pendentes = {
                        let Ok(conn) = state.conn() else { return };
                        match db::sync::movimentos_pendentes(&conn) {
                            Ok(p) => p,
                            Err(e) => {
                                log::warn!("Falha ao preparar sincronizacao com o Turso: {e}");
                                return;
                            }
                        }
                    };
                    match db::sync::enviar_para_turso(&url, &token, &pendentes).await {
                        Ok(enviados) => {
                            if let Ok(conn) = state.conn() {
                                if let Err(e) = db::sync::marcar_sincronizado(&conn, &enviados) {
                                    log::warn!(
                                        "Falha ao marcar lancamentos como sincronizados: {e}"
                                    );
                                }
                            }
                            log::info!(
                                "Sincronizacao com o Turso: {} lancamento(s) enviados.",
                                enviados.len()
                            );
                        }
                        Err(e) => log::warn!("Falha na sincronizacao com o Turso: {e}"),
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::status_commands::get_status,
            commands::auth_commands::setup_primeiro_usuario,
            commands::auth_commands::login,
            commands::auth_commands::logout,
            commands::auth_commands::listar_usuarios,
            commands::auth_commands::criar_usuario,
            commands::movimento_commands::criar_movimento,
            commands::movimento_commands::estornar_movimento,
            commands::movimento_commands::listar_movimentos_do_dia,
            commands::movimento_commands::sugestoes_descricao,
            commands::movimento_commands::buscar_historico,
            commands::fechamento_commands::fechar_dia,
            commands::fechamento_commands::buscar_fechamento_do_dia,
            commands::sync_commands::sincronizar_agora,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
