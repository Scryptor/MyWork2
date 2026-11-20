use crate::config::ServerError;
use mw_core::{Actor, Core, NewUser, User};
use mw_domain::permission::Permission;
use std::io::BufRead;

/// Создаёт первого администратора от имени консоли сервера.
/// Роль — первая по номеру с правом «Пользователи и права».
pub async fn create_admin(
    core: &Core,
    login: &str,
    display_name: &str,
    password: &str,
) -> Result<User, ServerError> {
    let actor = Actor::unrestricted("Консоль сервера");
    let mut roles = core.list_roles(&actor).await?;
    roles.sort_by_key(|role| role.id);
    let role = roles
        .into_iter()
        .find(|role| role.permissions.contains(Permission::UsersManage))
        .ok_or_else(|| ServerError::Config("нет роли с правом «Пользователи и права»".into()))?;
    Ok(core
        .create_user(
            &actor,
            NewUser {
                login: login.to_string(),
                display_name: display_name.to_string(),
                role_id: role.id,
                password: password.to_string(),
                can_sign_in: true,
                is_master: false,
            },
        )
        .await?)
}

/// Первая строка ввода без перевода строки; пустой пароль — ошибка.
pub fn read_password(input: &mut impl BufRead) -> Result<String, ServerError> {
    let mut line = String::new();
    input
        .read_line(&mut line)
        .map_err(|source| ServerError::Io {
            context: "не удалось прочитать пароль".into(),
            source,
        })?;
    let password = line.trim_end_matches(['\r', '\n']).to_string();
    if password.is_empty() {
        return Err(ServerError::Config("пароль не может быть пустым".into()));
    }
    Ok(password)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_password_takes_first_line() {
        assert_eq!(
            read_password(&mut "секрет\nлишнее".as_bytes()).unwrap(),
            "секрет"
        );
        assert_eq!(
            read_password(&mut "секрет\r\n".as_bytes()).unwrap(),
            "секрет"
        );
        assert!(read_password(&mut "\n".as_bytes()).is_err());
    }
}
