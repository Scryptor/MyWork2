use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// Логин; пустой у пользователя без логина (тогда `login_key` — [`no_login_key`]).
    pub login: String,
    pub login_key: String,
    pub display_name: String,
    pub role_id: i64,
    /// Пустой, если пароль не задан (у пользователя без входа).
    pub password_hash: String,
    /// `argon2` или `legacy_sha1`.
    pub password_scheme: String,
    pub active: bool,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub version: i64,
    /// Может входить в программу. Пользователь без входа (мастер, который в программе не
    /// работает) не входит ни с каким паролем и не считается ни в режиме без входа, ни в защите
    /// от потери доступа.
    pub can_sign_in: bool,
    /// Отметка «Мастер»: пользователя можно выбрать мастером заказа.
    pub is_master: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

/// Начало `login_key` пользователя без логина.
pub const NO_LOGIN_KEY_PREFIX: &str = "NO-LOGIN:";

/// Уникальный `login_key` пользователя без логина: начало [`NO_LOGIN_KEY_PREFIX`] и 32 случайные
/// шестнадцатеричные цифры. В ключе есть заглавные буквы, а ключ набранного логина
/// (`normalize_search`) их не содержит, поэтому ни с одним логином он не совпадает.
pub fn no_login_key() -> Result<String, DbErr> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes)
        .map_err(|e| DbErr::Custom(format!("нет случайных байт для ключа логина: {e}")))?;
    Ok(format!("{NO_LOGIN_KEY_PREFIX}{}", hex::encode(bytes)))
}
