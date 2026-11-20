//! Мастер заказа — пользователь с отметкой «Мастер»; в заказ записывается и его имя (снимок).

use mw_db::entity::user;
use mw_domain::validation::FieldError;
use sea_orm::{ConnectionTrait, EntityTrait};

use crate::CoreError;
use crate::auth::shown_name;

/// Мастера нет, он отключён или без отметки «Мастер».
const MASTER_UNAVAILABLE: &str = "Мастер не найден или отключён";

/// Ошибка поля `field`: такого мастера выбрать нельзя.
pub(crate) fn master_unavailable(field: &'static str) -> FieldError {
    FieldError::with_message(field, "invalid", MASTER_UNAVAILABLE)
}

/// Имя мастера `id` для нового выбора: активный пользователь с отметкой «Мастер» — его имя
/// (отображаемое, а если пустое — логин); иначе `None`.
pub(crate) async fn available_master(
    conn: &impl ConnectionTrait,
    id: i64,
) -> Result<Option<String>, CoreError> {
    Ok(user::Entity::find_by_id(id)
        .one(conn)
        .await?
        .filter(|u| u.is_master && u.active)
        .map(|u| shown_name(&u).to_owned()))
}
