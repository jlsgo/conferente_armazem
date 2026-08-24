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
    let solicitante = buscar_usuario(conn, solicitante_id)?;
    if solicitante.papel != "gestor" {
        return Err(AppError::Validation(
            "Somente um gestor pode cadastrar novos usuarios.".into(),
        ));
    }
    criar_usuario(conn, novo)
}

pub fn login(conn: &Connection, login_input: &str, senha: &str) -> AppResult<Usuario> {
    let mut stmt = conn.prepare(
        "SELECT id, nome, login, senha_hash, armazem_id, papel, ativo
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
            ))
        })
        .optional()?;

    let (id, nome, login_db, senha_hash, armazem_id, papel, ativo) =
        encontrado.ok_or(AppError::CredenciaisInvalidas)?;

    if !verify_password(senha, &senha_hash) {
        return Err(AppError::CredenciaisInvalidas);
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
}
