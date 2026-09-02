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
            // maquina via turso.txt): um loop de segundo plano que tenta a
            // cada INTERVALO_SINCRONIZACAO, pela vida inteira do processo -
            // nao exige sessao/login nem gestor (roda antes de qualquer
            // login e continua rodando com um conferente logado), pra nao
            // depender de um gestor abrir o Dashboard pra a fila ser
            // reenviada (ver docs/ARQUITETURA.md). Reconfere turso.txt a
            // cada iteracao, nao so uma vez no boot - configurar o arquivo
            // numa maquina ja aberta passa a funcionar sem reiniciar o app.
            // Nunca trava a abertura do app se nao tiver internet ou o
            // arquivo nao existir.
            const INTERVALO_SINCRONIZACAO: std::time::Duration = std::time::Duration::from_secs(60);
            let app_handle = app.handle().clone();
            let diretorio_dados_sync = diretorio_dados.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    if let Some((url, token)) = db::sync::ler_config_turso(&diretorio_dados_sync) {
                        db::sync::tentar_sincronizar_uma_vez(&app_handle, &url, &token).await;
                    }
                    tokio::time::sleep(INTERVALO_SINCRONIZACAO).await;
                }
            });

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
            commands::movimento_commands::verificar_retirada_pendente,
            commands::movimento_commands::buscar_reparos_em_aberto,
            commands::movimento_commands::buscar_reparos_concluidos,
            commands::fechamento_commands::fechar_dia,
            commands::fechamento_commands::buscar_fechamento_do_dia,
            commands::sync_commands::sincronizar_agora,
            commands::sync_commands::status_sincronizacao,
            commands::sync_commands::buscar_transferencias_pendentes,
            commands::sync_commands::confirmar_recebimento,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
