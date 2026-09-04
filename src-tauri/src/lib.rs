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

            let diretorio_backups = diretorio_dados.join("backups");
            let mut arquivos_do_dia: Vec<std::path::PathBuf> = Vec::new();

            match db::backup::backup_automatico(&conn, &diretorio_dados) {
                Ok(caminho) => arquivos_do_dia.push(caminho),
                Err(e) => log::warn!("Falha ao fazer backup automatico do banco: {e}"),
            }

            // Backup externo (pendrive/HD): so acontece se `backup_externo.txt`
            // estiver configurado nesta maquina e a unidade estiver conectada
            // agora - melhor-esforco, nunca trava a abertura do app.
            let destino_externo = db::backup::ler_destino_externo(&diretorio_dados);
            if let Some(destino) = &destino_externo {
                if let Err(e) = db::backup::backup_externo(&conn, &diretorio_dados, destino) {
                    log::warn!("Falha ao fazer backup externo do banco: {e}");
                }
            }

            // Copias dos arquivos de config (turso.txt, backup_externo.txt) ja
            // foram feitas por backup_automatico/backup_externo acima, dentro
            // da pasta de backups local - incluir aqui pra tambem irem pro
            // backup offsite (S3) mais abaixo.
            for nome in [
                db::sync::NOME_ARQUIVO_CONFIG_TURSO,
                db::backup::NOME_ARQUIVO_CONFIG_EXTERNO,
            ] {
                let copia = diretorio_backups.join(nome);
                if copia.exists() {
                    arquivos_do_dia.push(copia);
                }
            }

            let data_hoje: Option<String> = conn
                .query_row("SELECT date('now', 'localtime')", [], |r| r.get(0))
                .ok();

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

            // Backup completo: export do Turso (dump de movimentos_consolidados
            // - a unica copia offline dessa tabela, ver docs/ARQUITETURA.md) e
            // upload offsite pro S3 (se `backup_nuvem.txt` estiver configurado).
            // Roda uma vez por abertura real do app (mesma cadencia do backup
            // local/externo acima), nao um loop - granularidade diaria basta
            // pra um backup. As duas etapas sao independentes: falha numa nao
            // impede a outra, e nenhuma delas trava a abertura do app.
            tauri::async_runtime::spawn(async move {
                let mut arquivos_do_dia = arquivos_do_dia;

                if let (Some((url, token)), Some(data)) =
                    (db::sync::ler_config_turso(&diretorio_dados), data_hoje)
                {
                    match db::sync::exportar_consolidado(&url, &token, &diretorio_backups, &data)
                        .await
                    {
                        Ok(caminho) => arquivos_do_dia.push(caminho),
                        Err(e) => {
                            log::warn!("Falha ao exportar a tabela consolidada do Turso: {e}")
                        }
                    }
                    if let Some(destino) = &destino_externo {
                        if let Err(e) =
                            db::sync::exportar_consolidado(&url, &token, destino, &data).await
                        {
                            log::warn!(
                                "Falha ao exportar a tabela consolidada do Turso pro destino externo: {e}"
                            );
                        }
                    }
                }

                if let Some(config) = db::backup_nuvem::ler_config_nuvem(&diretorio_dados) {
                    db::backup_nuvem::enviar_backups_do_dia(&config, &arquivos_do_dia).await;
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
            commands::sync_commands::recusar_recebimento,
            commands::sync_commands::buscar_transferencias_recusadas,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
