use argon2::password_hash::{PasswordHash, SaltString};
use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use rand_core::OsRng;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use super::errors::{AppError, AppResult};

#[derive(Debug, Clone, Serialize)]
pub struct Usuario {
    pub id: i64,
    pub nome: String,
    pub login: String,
    pub armazem_id: Option<i64>,
    pub papel: String,
    pub ativo: bool,
}

pub struct NovoUsuario<'a> {
    pub nome: &'a str,
    pub login: &'a str,
    pub senha: &'a str,
    pub armazem_id: Option<i64>,
    pub papel: &'a str,
}

fn hash_password(senha: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(senha.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|_| AppError::Hashing)
}

fn verify_password(senha: &str, senha_hash: &str) -> bool {
    let Ok(hash_parseado) = PasswordHash::new(senha_hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(senha.as_bytes(), &hash_parseado)
        .is_ok()
}

const PAPEIS_VALIDOS: [&str; 2] = ["conferente", "gestor"];

fn validar_novo_usuario(novo: &NovoUsuario) -> AppResult<()> {
    if novo.nome.trim().is_empty() {
        return Err(AppError::Validation("Informe o nome completo.".into()));
    }
    if novo.login.trim().is_empty() {
        return Err(AppError::Validation("Informe o usuario de acesso.".into()));
    }
    if novo.senha.chars().count() < 6 {
        return Err(AppError::Validation(
            "A senha precisa ter pelo menos 6 caracteres.".into(),
        ));
    }
    if !PAPEIS_VALIDOS.contains(&novo.papel) {
        return Err(AppError::Validation(format!(
            "Papel invalido: {}",
            novo.papel
        )));
    }
    Ok(())
}

pub fn contar_usuarios(conn: &Connection) -> AppResult<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM usuarios", [], |r| r.get(0))?)
}

fn mapear_usuario(r: &rusqlite::Row) -> rusqlite::Result<Usuario> {
    Ok(Usuario {
        id: r.get(0)?,
        nome: r.get(1)?,
        login: r.get(2)?,
        armazem_id: r.get(3)?,
        papel: r.get(4)?,
        ativo: r.get(5)?,
    })
}

const COLUNAS_USUARIO: &str = "id, nome, login, armazem_id, papel, ativo";

/// Busca um usuario pelo id. Usado para checar o papel de quem esta
/// solicitando uma acao restrita (ex.: cadastrar outro usuario), nunca
/// confiando so no que o frontend diz que o usuario logado e.
pub fn buscar_usuario(conn: &Connection, id: i64) -> AppResult<Usuario> {
    conn.query_row(
        &format!("SELECT {COLUNAS_USUARIO} FROM usuarios WHERE id = ?1"),
        params![id],
        mapear_usuario,
    )
    .map_err(|_| AppError::Validation("Usuario nao encontrado.".into()))
}

/// Como `buscar_usuario`, mas tambem rejeita usuario desativado. Usado em todo
/// ponto de entrada que precisa confirmar "quem esta fazendo isso ainda pode
/// fazer isso" (criar movimento, fechar dia, estornar, cadastrar usuario) -
/// nunca confiando so no id vindo da sessao/payload sem reconferir no banco.
pub fn buscar_usuario_ativo(conn: &Connection, id: i64) -> AppResult<Usuario> {
    let usuario = buscar_usuario(conn, id)?;
    if !usuario.ativo {
        return Err(AppError::Validation("Usuario inativo.".into()));
    }
    Ok(usuario)
}

pub fn listar_usuarios(conn: &Connection, armazem_id: Option<i64>) -> AppResult<Vec<Usuario>> {
    let mut stmt = if armazem_id.is_some() {
        conn.prepare(&format!(
            "SELECT {COLUNAS_USUARIO} FROM usuarios WHERE ativo = 1 AND armazem_id = ?1 ORDER BY nome"
        ))?
    } else {
        conn.prepare(&format!(
            "SELECT {COLUNAS_USUARIO} FROM usuarios WHERE ativo = 1 ORDER BY nome"
        ))?
    };

    let usuarios = match armazem_id {
        Some(id) => stmt
            .query_map(params![id], mapear_usuario)?
            .collect::<Result<Vec<_>, _>>()?,
        None => stmt
            .query_map([], mapear_usuario)?
            .collect::<Result<Vec<_>, _>>()?,
    };

    Ok(usuarios)
}

pub fn criar_usuario(conn: &Connection, novo: NovoUsuario) -> AppResult<i64> {
    validar_novo_usuario(&novo)?;
    let senha_hash = hash_password(novo.senha)?;

    let resultado = conn.execute(
        "INSERT INTO usuarios (nome, login, senha_hash, armazem_id, papel)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            novo.nome.trim(),
            novo.login.trim(),
            senha_hash,
            novo.armazem_id,
            novo.papel
        ],
    );

    match resultado {
        Ok(_) => Ok(conn.last_insert_rowid()),
        Err(rusqlite::Error::SqliteFailure(err, _))
            if err.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            Err(AppError::Validation(
                "Esse nome de usuario ja existe.".into(),
            ))
        }
        Err(e) => Err(AppError::Database(e)),
    }
}

/// Cadastra um novo usuario, mas so se quem esta solicitando (`solicitante_id`)
/// for um gestor. A checagem de papel e feita aqui no dominio, no backend —
/// nunca confiando que o frontend so escondeu o botao de quem nao e gestor.
pub fn criar_usuario_como_gestor(
    conn: &Connection,
    solicitante_id: i64,
    novo: NovoUsuario,
) -> AppResult<i64> {
    let solicitante = buscar_usuario_ativo(conn, solicitante_id)?;
    if solicitante.papel != "gestor" {
        return Err(AppError::Validation(
            "Somente um gestor pode cadastrar novos usuarios.".into(),
        ));
    }
    if novo.papel == "gestor" {
        return Err(AppError::Validation(
            "Nao e permitido cadastrar outro gestor por aqui.".into(),
        ));
    }
    criar_usuario(conn, novo)
}

/// Como `listar_usuarios`, mas so se quem esta pedindo (`solicitante_id`) for
/// um gestor - hoje so a tela `Usuarios.tsx` (gestor-only na UI) chama isso,
/// mas o comando Tauri em si nao confiava so nisso antes desta checagem.
pub fn listar_usuarios_como_gestor(
    conn: &Connection,
    solicitante_id: i64,
    armazem_id: Option<i64>,
) -> AppResult<Vec<Usuario>> {
    let solicitante = buscar_usuario_ativo(conn, solicitante_id)?;
    if solicitante.papel != "gestor" {
        return Err(AppError::Validation(
            "Somente um gestor pode listar usuarios.".into(),
        ));
    }
    listar_usuarios(conn, armazem_id)
}

/// Tentativas erradas seguidas permitidas antes de comecar a bloquear -
/// gerar bloqueio ja na 1a tentativa penalizaria demais um simples erro de
/// digitacao da conferente.
const TENTATIVAS_LIVRES: i64 = 3;

/// Minutos de bloqueio para a N-esima tentativa errada (so chamada quando
/// `tentativas_falhas > TENTATIVAS_LIVRES`) - mesmo formato progressivo
/// (1/5/15/30, fixo em 60 dali pra frente) ja usado no backoff de sync
/// (`db::sync::calcular_backoff_minutos`), reescrito aqui porque `domain`
/// nao deve depender de `db` (o dominio e testado sozinho, sem infra).
fn calcular_bloqueio_minutos(tentativas_falhas: i64) -> i64 {
    match tentativas_falhas - TENTATIVAS_LIVRES {
        n if n <= 0 => 0,
        1 => 1,
        2 => 5,
        3 => 15,
        4 => 30,
        _ => 60,
    }
}

pub fn login(conn: &Connection, login_input: &str, senha: &str) -> AppResult<Usuario> {
    let mut stmt = conn.prepare(
        "SELECT id, nome, login, senha_hash, armazem_id, papel, ativo, tentativas_falhas, bloqueado_ate
         FROM usuarios WHERE login = ?1 AND ativo = 1",
    )?;

    let encontrado = stmt
        .query_row(params![login_input], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<i64>>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, bool>(6)?,
                r.get::<_, i64>(7)?,
                r.get::<_, Option<String>>(8)?,
            ))
        })
        .optional()?;

    let (
        id,
        nome,
        login_db,
        senha_hash,
        armazem_id,
        papel,
        ativo,
        tentativas_falhas,
        bloqueado_ate,
    ) = encontrado.ok_or(AppError::CredenciaisInvalidas)?;

    let agora: String = conn.query_row("SELECT datetime('now', 'localtime')", [], |r| r.get(0))?;
    if bloqueado_ate
        .as_deref()
        .is_some_and(|ate| ate > agora.as_str())
    {
        return Err(AppError::ContaBloqueada);
    }

    if !verify_password(senha, &senha_hash) {
        let tentativas = tentativas_falhas + 1;
        let bloqueio_minutos = calcular_bloqueio_minutos(tentativas);
        if bloqueio_minutos > 0 {
            conn.execute(
                "UPDATE usuarios SET tentativas_falhas = ?1,
                    bloqueado_ate = datetime('now', 'localtime', '+' || ?2 || ' minutes')
                 WHERE id = ?3",
                params![tentativas, bloqueio_minutos, id],
            )?;
        } else {
            conn.execute(
                "UPDATE usuarios SET tentativas_falhas = ?1 WHERE id = ?2",
                params![tentativas, id],
            )?;
        }
        return Err(AppError::CredenciaisInvalidas);
    }

    if tentativas_falhas > 0 || bloqueado_ate.is_some() {
        conn.execute(
            "UPDATE usuarios SET tentativas_falhas = 0, bloqueado_ate = NULL WHERE id = ?1",
            params![id],
        )?;
    }

    Ok(Usuario {
        id,
        nome,
        login: login_db,
        armazem_id,
        papel,
        ativo,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn conexao_de_teste() -> Connection {
        db::abrir_em_memoria().unwrap()
    }

    fn id_do_armazem(conn: &Connection, codigo: &str) -> i64 {
        conn.query_row("SELECT id FROM armazens WHERE codigo = ?1", [codigo], |r| {
            r.get(0)
        })
        .unwrap()
    }

    #[test]
    fn cria_usuario_e_permite_login_com_senha_correta() {
        let conn = conexao_de_teste();
        let armazem_id = id_do_armazem(&conn, "A4");

        criar_usuario(
            &conn,
            NovoUsuario {
                nome: "Karol Pereira",
                login: "karol",
                senha: "senha123",
                armazem_id: Some(armazem_id),
                papel: "gestor",
            },
        )
        .expect("deveria criar o usuario");

        let usuario = login(&conn, "karol", "senha123").expect("login deveria funcionar");
        assert_eq!(usuario.nome, "Karol Pereira");
        assert_eq!(usuario.papel, "gestor");
    }

    #[test]
    fn rejeita_login_com_senha_errada() {
        let conn = conexao_de_teste();
        criar_usuario(
            &conn,
            NovoUsuario {
                nome: "Alice",
                login: "alice",
                senha: "senha123",
                armazem_id: None,
                papel: "conferente",
            },
        )
        .unwrap();

        let resultado = login(&conn, "alice", "senha-errada");
        assert!(matches!(resultado, Err(AppError::CredenciaisInvalidas)));
    }

    #[test]
    fn rejeita_login_de_usuario_inexistente() {
        let conn = conexao_de_teste();
        let resultado = login(&conn, "ninguem", "qualquer");
        assert!(matches!(resultado, Err(AppError::CredenciaisInvalidas)));
    }

    #[test]
    fn rejeita_senha_curta_ao_criar_usuario() {
        let conn = conexao_de_teste();
        let resultado = criar_usuario(
            &conn,
            NovoUsuario {
                nome: "Teste",
                login: "teste",
                senha: "123",
                armazem_id: None,
                papel: "conferente",
            },
        );
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    #[test]
    fn rejeita_papel_invalido_ao_criar_usuario() {
        let conn = conexao_de_teste();
        let resultado = criar_usuario(
            &conn,
            NovoUsuario {
                nome: "Teste",
                login: "teste2",
                senha: "senha123",
                armazem_id: None,
                papel: "admin",
            },
        );
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    #[test]
    fn rejeita_login_duplicado() {
        let conn = conexao_de_teste();
        let novo = || NovoUsuario {
            nome: "Duplicado",
            login: "dup",
            senha: "senha123",
            armazem_id: None,
            papel: "conferente",
        };
        criar_usuario(&conn, novo()).unwrap();
        let resultado = criar_usuario(&conn, novo());
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    #[test]
    fn gestor_nao_pode_cadastrar_outro_gestor() {
        let conn = conexao_de_teste();
        let gestor_id = criar_usuario(
            &conn,
            NovoUsuario {
                nome: "Brenda",
                login: "brenda",
                senha: "senha123",
                armazem_id: None,
                papel: "gestor",
            },
        )
        .unwrap();

        let resultado = criar_usuario_como_gestor(
            &conn,
            gestor_id,
            NovoUsuario {
                nome: "Karol",
                login: "karol",
                senha: "senha123",
                armazem_id: None,
                papel: "gestor",
            },
        );
        assert!(matches!(resultado, Err(AppError::Validation(_))));
        assert_eq!(listar_usuarios(&conn, None).unwrap().len(), 1);
    }

    #[test]
    fn gestor_pode_cadastrar_novo_usuario() {
        let conn = conexao_de_teste();
        let armazem_id = id_do_armazem(&conn, "A4");
        let gestor_id = criar_usuario(
            &conn,
            NovoUsuario {
                nome: "Brenda",
                login: "brenda",
                senha: "senha123",
                armazem_id: Some(armazem_id),
                papel: "gestor",
            },
        )
        .unwrap();

        let resultado = criar_usuario_como_gestor(
            &conn,
            gestor_id,
            NovoUsuario {
                nome: "Karol",
                login: "karol",
                senha: "senha123",
                armazem_id: Some(armazem_id),
                papel: "conferente",
            },
        );
        assert!(resultado.is_ok());
        assert_eq!(listar_usuarios(&conn, None).unwrap().len(), 2);
    }

    #[test]
    fn gestor_inativo_nao_pode_cadastrar_novo_usuario() {
        let conn = conexao_de_teste();
        let gestor_id = criar_usuario(
            &conn,
            NovoUsuario {
                nome: "Brenda",
                login: "brenda",
                senha: "senha123",
                armazem_id: None,
                papel: "gestor",
            },
        )
        .unwrap();
        conn.execute(
            "UPDATE usuarios SET ativo = 0 WHERE id = ?1",
            params![gestor_id],
        )
        .unwrap();

        let resultado = criar_usuario_como_gestor(
            &conn,
            gestor_id,
            NovoUsuario {
                nome: "Karol",
                login: "karol",
                senha: "senha123",
                armazem_id: None,
                papel: "conferente",
            },
        );
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    #[test]
    fn login_rejeita_usuario_desativado_mesmo_com_senha_correta() {
        let conn = conexao_de_teste();
        let usuario_id = criar_usuario(
            &conn,
            NovoUsuario {
                nome: "Karol",
                login: "karol",
                senha: "senha123",
                armazem_id: None,
                papel: "conferente",
            },
        )
        .unwrap();
        conn.execute(
            "UPDATE usuarios SET ativo = 0 WHERE id = ?1",
            params![usuario_id],
        )
        .unwrap();

        // Mesma mensagem generica de "usuario ou senha invalidos" que um
        // login inexistente - nao deve dar nenhuma pista de que o usuario
        // existe mas foi desativado.
        let resultado = login(&conn, "karol", "senha123");
        assert!(matches!(resultado, Err(AppError::CredenciaisInvalidas)));
    }

    #[test]
    fn login_rejeita_senha_vazia() {
        let conn = conexao_de_teste();
        criar_usuario(
            &conn,
            NovoUsuario {
                nome: "Alice",
                login: "alice",
                senha: "senha123",
                armazem_id: None,
                papel: "conferente",
            },
        )
        .unwrap();

        let resultado = login(&conn, "alice", "");
        assert!(matches!(resultado, Err(AppError::CredenciaisInvalidas)));
    }

    #[test]
    fn senha_nunca_e_armazenada_em_texto_puro_e_usa_salt_diferente_a_cada_vez() {
        let conn = conexao_de_teste();
        criar_usuario(
            &conn,
            NovoUsuario {
                nome: "Alice",
                login: "alice",
                senha: "senha-super-secreta",
                armazem_id: None,
                papel: "conferente",
            },
        )
        .unwrap();
        criar_usuario(
            &conn,
            NovoUsuario {
                nome: "Bia",
                login: "bia",
                senha: "senha-super-secreta",
                armazem_id: None,
                papel: "conferente",
            },
        )
        .unwrap();

        let hashes: Vec<String> = conn
            .prepare("SELECT senha_hash FROM usuarios ORDER BY login")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        for hash in &hashes {
            assert_ne!(hash, "senha-super-secreta");
            assert!(!hash.contains("senha-super-secreta"));
            assert!(hash.starts_with("$argon2"));
        }
        // Mesma senha em duas contas diferentes precisa gerar hashes
        // diferentes (salt aleatorio) - senao um vazamento do banco
        // revelaria quais contas compartilham senha.
        assert_ne!(hashes[0], hashes[1]);
    }

    #[test]
    fn criar_usuario_como_gestor_rejeita_solicitante_inexistente() {
        let conn = conexao_de_teste();
        let resultado = criar_usuario_como_gestor(
            &conn,
            999_999,
            NovoUsuario {
                nome: "Invasor",
                login: "invasor",
                senha: "senha123",
                armazem_id: None,
                papel: "gestor",
            },
        );
        assert!(matches!(resultado, Err(AppError::Validation(_))));
        assert_eq!(listar_usuarios(&conn, None).unwrap().len(), 0);
    }

    #[test]
    fn conferente_nao_pode_listar_usuarios() {
        let conn = conexao_de_teste();
        let conferente_id = criar_usuario(
            &conn,
            NovoUsuario {
                nome: "Karol",
                login: "karol",
                senha: "senha123",
                armazem_id: None,
                papel: "conferente",
            },
        )
        .unwrap();

        let resultado = listar_usuarios_como_gestor(&conn, conferente_id, None);
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    #[test]
    fn gestor_pode_listar_usuarios() {
        let conn = conexao_de_teste();
        let gestor_id = criar_usuario(
            &conn,
            NovoUsuario {
                nome: "Brenda",
                login: "brenda",
                senha: "senha123",
                armazem_id: None,
                papel: "gestor",
            },
        )
        .unwrap();

        let resultado = listar_usuarios_como_gestor(&conn, gestor_id, None).unwrap();
        assert_eq!(resultado.len(), 1);
    }

    #[test]
    fn conferente_nao_pode_cadastrar_novo_usuario() {
        let conn = conexao_de_teste();
        let conferente_id = criar_usuario(
            &conn,
            NovoUsuario {
                nome: "Karol",
                login: "karol",
                senha: "senha123",
                armazem_id: None,
                papel: "conferente",
            },
        )
        .unwrap();

        let resultado = criar_usuario_como_gestor(
            &conn,
            conferente_id,
            NovoUsuario {
                nome: "Outra",
                login: "outra",
                senha: "senha123",
                armazem_id: None,
                papel: "conferente",
            },
        );
        assert!(matches!(resultado, Err(AppError::Validation(_))));
    }

    // --- Bloqueio progressivo apos tentativas erradas de login ---

    #[test]
    fn calcular_bloqueio_minutos_nao_bloqueia_dentro_das_tentativas_livres() {
        assert_eq!(calcular_bloqueio_minutos(1), 0);
        assert_eq!(calcular_bloqueio_minutos(2), 0);
        assert_eq!(calcular_bloqueio_minutos(TENTATIVAS_LIVRES), 0);
    }

    #[test]
    fn calcular_bloqueio_minutos_escala_apos_as_tentativas_livres() {
        assert_eq!(calcular_bloqueio_minutos(TENTATIVAS_LIVRES + 1), 1);
        assert_eq!(calcular_bloqueio_minutos(TENTATIVAS_LIVRES + 2), 5);
        assert_eq!(calcular_bloqueio_minutos(TENTATIVAS_LIVRES + 3), 15);
        assert_eq!(calcular_bloqueio_minutos(TENTATIVAS_LIVRES + 4), 30);
        assert_eq!(calcular_bloqueio_minutos(TENTATIVAS_LIVRES + 5), 60);
        assert_eq!(calcular_bloqueio_minutos(TENTATIVAS_LIVRES + 20), 60);
    }

    #[test]
    fn erros_de_senha_dentro_da_margem_livre_nao_bloqueiam_a_conta() {
        let conn = conexao_de_teste();
        criar_usuario(
            &conn,
            NovoUsuario {
                nome: "Karol",
                login: "karol",
                senha: "senha123",
                armazem_id: None,
                papel: "conferente",
            },
        )
        .unwrap();

        for _ in 0..TENTATIVAS_LIVRES {
            let resultado = login(&conn, "karol", "senha-errada");
            assert!(matches!(resultado, Err(AppError::CredenciaisInvalidas)));
        }

        // A senha certa ainda deve funcionar - nenhuma das tentativas
        // anteriores chegou a bloquear a conta.
        assert!(login(&conn, "karol", "senha123").is_ok());
    }

    #[test]
    fn excesso_de_tentativas_erradas_bloqueia_a_conta_mesmo_com_senha_certa_depois() {
        let conn = conexao_de_teste();
        criar_usuario(
            &conn,
            NovoUsuario {
                nome: "Karol",
                login: "karol",
                senha: "senha123",
                armazem_id: None,
                papel: "conferente",
            },
        )
        .unwrap();

        for _ in 0..=TENTATIVAS_LIVRES {
            let _ = login(&conn, "karol", "senha-errada");
        }

        let resultado = login(&conn, "karol", "senha123");
        assert!(matches!(resultado, Err(AppError::ContaBloqueada)));
    }

    #[test]
    fn conta_desbloqueia_sozinha_apos_o_prazo_passar() {
        let conn = conexao_de_teste();
        let usuario_id = criar_usuario(
            &conn,
            NovoUsuario {
                nome: "Karol",
                login: "karol",
                senha: "senha123",
                armazem_id: None,
                papel: "conferente",
            },
        )
        .unwrap();

        for _ in 0..=TENTATIVAS_LIVRES {
            let _ = login(&conn, "karol", "senha-errada");
        }
        assert!(matches!(
            login(&conn, "karol", "senha123"),
            Err(AppError::ContaBloqueada)
        ));

        // Simula o prazo de bloqueio ja tendo passado (sem esperar de
        // verdade o relogio andar).
        conn.execute(
            "UPDATE usuarios SET bloqueado_ate = datetime('now', 'localtime', '-1 minutes') WHERE id = ?1",
            params![usuario_id],
        )
        .unwrap();

        assert!(login(&conn, "karol", "senha123").is_ok());
    }

    #[test]
    fn login_com_sucesso_zera_o_contador_de_tentativas() {
        let conn = conexao_de_teste();
        let usuario_id = criar_usuario(
            &conn,
            NovoUsuario {
                nome: "Karol",
                login: "karol",
                senha: "senha123",
                armazem_id: None,
                papel: "conferente",
            },
        )
        .unwrap();

        let _ = login(&conn, "karol", "senha-errada");
        let _ = login(&conn, "karol", "senha-errada");
        assert!(login(&conn, "karol", "senha123").is_ok());

        let tentativas: i64 = conn
            .query_row(
                "SELECT tentativas_falhas FROM usuarios WHERE id = ?1",
                params![usuario_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tentativas, 0);

        // Depois do reset, uma unica tentativa errada nao deve bloquear de
        // novo (a margem livre volta a valer).
        let _ = login(&conn, "karol", "senha-errada");
        assert!(login(&conn, "karol", "senha123").is_ok());
    }
}
