//! Группы доступа и пользователи старой базы: роли, логины и хэши паролей.

use std::collections::BTreeMap;

use mw_core::{ImportedLegacyRow, ImportedUser, LegacyTarget, NewRole};
use mw_domain::permission::{Permission, PermissionSet};
use mw_domain::search::normalize_search;
use serde_json::Value;

use crate::parse::parse_flag;
use crate::report::{WarningCode, Warnings};
use crate::source::LegacyDb;

const ADMIN_ROLE: &str = "Администратор";
const NO_RIGHTS_ROLE: &str = "Без прав";
const SUFFIX: &str = "MyWork 2";

/// Флаг группы → права.
const FLAG_PERMISSIONS: &[(&str, &[Permission])] = &[
    ("addZapis", &[Permission::OrdersCreate]),
    ("saveZapis", &[Permission::OrdersEdit]),
    ("delZapis", &[Permission::OrdersDelete]),
    ("editDates", &[Permission::OrderDatesEdit]),
    ("graf", &[Permission::ReportsView]),
    ("stock", &[Permission::StockView]),
    ("stockAdd", &[Permission::StockCreate]),
    ("stockEdit", &[Permission::StockEdit]),
    ("stockDel", &[Permission::StockDelete]),
    (
        "clients",
        &[Permission::ClientsView, Permission::ClientsEdit],
    ),
    ("clientAdd", &[Permission::ClientsCreate]),
    ("clientDel", &[Permission::ClientsDelete]),
    ("clientConcat", &[Permission::ClientsMerge]),
    (
        "settings",
        &[
            Permission::SettingsManage,
            Permission::UsersManage,
            Permission::AuditView,
        ],
    ),
    // `sms` и `dates` в MyWork 3 соответствия не имеют.
    ("sms", &[]),
    ("dates", &[]),
];

/// Свободное имя: само, затем с суффиксом « (MyWork 2)», « (MyWork 2, 2)»…
fn free_name(base: &str, taken: &mut BTreeMap<String, ()>) -> String {
    let mut name = base.to_owned();
    let mut n = 1;
    while taken.contains_key(&normalize_search(&name)) {
        n += 1;
        name = if n == 2 {
            format!("{base} ({SUFFIX})")
        } else {
            format!("{base} ({SUFFIX}, {})", n - 1)
        };
    }
    taken.insert(normalize_search(&name), ());
    name
}

/// Следующий логин: сам, затем « (2)», « (3)»…
fn free_login(base: &str, taken: &mut BTreeMap<String, ()>) -> String {
    let mut login = base.to_owned();
    let mut n = 1;
    while taken.contains_key(&normalize_search(&login)) {
        n += 1;
        login = format!("{base} ({n})");
    }
    taken.insert(normalize_search(&login), ());
    login
}

fn group_permissions(
    table: &crate::source::LegacyTable,
    row: &[crate::source::LegacyValue],
    id: &str,
    w: &mut Warnings,
) -> PermissionSet {
    let mut set = PermissionSet::empty();
    for (flag, permissions) in FLAG_PERMISSIONS {
        let raw = table.text(row, flag);
        let on = parse_flag(&raw).unwrap_or_else(|| {
            w.push(
                WarningCode::FlagUnrecognized,
                "GroupDostup",
                id,
                flag,
                &raw,
                "Флаг группы не распознан, право не выдано.",
            );
            false
        });
        if on {
            for p in *permissions {
                set.insert(*p);
            }
        }
    }
    set
}

fn is_sha1_hex(value: &str) -> bool {
    value.len() == 40 && value.chars().all(|c| c.is_ascii_hexdigit())
}

struct Roles {
    list: Vec<NewRole>,
    taken: BTreeMap<String, ()>,
    /// id группы MyWork 2 → название роли.
    by_group: BTreeMap<i64, String>,
}

fn convert_groups(db: &LegacyDb, w: &mut Warnings) -> (Roles, Vec<ImportedLegacyRow>) {
    let mut roles = Roles {
        list: vec![NewRole {
            name: ADMIN_ROLE.to_owned(),
            permissions: PermissionSet::all(),
        }],
        taken: BTreeMap::from([(normalize_search(ADMIN_ROLE), ())]),
        by_group: BTreeMap::new(),
    };
    let mut rows = Vec::new();
    let Some(table) = db.table("GroupDostup") else {
        return (roles, rows);
    };
    let mut sorted: Vec<_> = table.rows.iter().collect();
    sorted.sort_by_key(|r| {
        table
            .text(r, "id")
            .trim()
            .parse::<i64>()
            .unwrap_or(i64::MAX)
    });
    for row in sorted {
        let id = table.text(row, "id");
        let id = id.trim();
        let legacy_name = table.text(row, "grName");
        let base = match legacy_name.trim() {
            "" => format!("Группа {id}"),
            n => n.to_owned(),
        };
        let name = free_name(&base, &mut roles.taken);
        if name != legacy_name.trim() {
            w.push(
                WarningCode::RoleRenamed,
                "GroupDostup",
                id,
                "grName",
                legacy_name.trim(),
                &format!("Группа переименована в «{name}»."),
            );
        }
        let permissions = group_permissions(table, row, id, w);
        if let Ok(n) = id.parse::<i64>() {
            roles.by_group.insert(n, name.clone());
        }
        rows.push(ImportedLegacyRow {
            source: "GroupDostup".to_owned(),
            source_key: id.to_owned(),
            target: Some(LegacyTarget::Role(name.clone())),
            raw: table.row_json(row),
        });
        roles.list.push(NewRole { name, permissions });
    }
    (roles, rows)
}

/// Роль «Без прав» — только если она нужна, с уникальным именем.
fn no_rights_role(roles: &mut Roles) -> String {
    if let Some(existing) = roles.by_group.get(&-1) {
        return existing.clone();
    }
    let name = free_name(NO_RIGHTS_ROLE, &mut roles.taken);
    roles.list.push(NewRole {
        name: name.clone(),
        permissions: PermissionSet::empty(),
    });
    roles.by_group.insert(-1, name.clone());
    name
}

/// Роли, пользователи и следы строк `GroupDostup` и `Users`.
pub(crate) fn convert_access(
    db: &LegacyDb,
    w: &mut Warnings,
) -> (Vec<NewRole>, Vec<ImportedUser>, Vec<ImportedLegacyRow>) {
    let (mut roles, mut rows) = convert_groups(db, w);
    let mut users = Vec::new();
    if let Some(table) = db.table("Users") {
        let mut sorted: Vec<_> = table.rows.iter().collect();
        sorted.sort_by_key(|r| {
            table
                .text(r, "id")
                .trim()
                .parse::<i64>()
                .unwrap_or(i64::MAX)
        });
        let mut logins = BTreeMap::new();
        for row in sorted {
            let id = table.text(row, "id");
            let id = id.trim();
            let name = table.text(row, "name");
            let base = match name.trim() {
                "" => format!("Пользователь {id}"),
                n => n.to_owned(),
            };
            let login = free_login(&base, &mut logins);
            if login != name.trim() {
                w.push(
                    WarningCode::LoginRenamed,
                    "Users",
                    id,
                    "name",
                    name.trim(),
                    &format!("Логин совпал с другим без учёта регистра, новый логин «{login}»."),
                );
            }
            let group = table.text(row, "id_gruppi_dostupa");
            let role = match group
                .trim()
                .parse::<i64>()
                .ok()
                .and_then(|g| roles.by_group.get(&g))
            {
                Some(role) => role.clone(),
                None => {
                    w.push(
                        WarningCode::UserGroupMissing,
                        "Users",
                        id,
                        "id_gruppi_dostupa",
                        group.trim(),
                        "Группа пользователя не найдена, выдана роль без прав.",
                    );
                    no_rights_role(&mut roles)
                }
            };
            let hash = table.text(row, "user_pwd").trim().to_lowercase();
            let legacy_sha1 = if is_sha1_hex(&hash) {
                hash
            } else {
                w.push(
                    WarningCode::UserPasswordInvalid,
                    "Users",
                    id,
                    "user_pwd",
                    "",
                    "Пароль не похож на SHA-1, не перенесён: пользователю нужно задать новый пароль.",
                );
                String::new()
            };
            let mut raw = table.row_json(row);
            if let Value::Object(map) = &mut raw {
                for (key, value) in map.iter_mut() {
                    if key.eq_ignore_ascii_case("user_pwd") {
                        *value = Value::String("***".to_owned());
                    }
                }
            }
            rows.push(ImportedLegacyRow {
                source: "Users".to_owned(),
                source_key: id.to_owned(),
                target: Some(LegacyTarget::User(login.clone())),
                raw,
            });
            users.push(ImportedUser {
                login,
                role,
                legacy_sha1,
            });
        }
    }
    grant_users_manage(&mut roles, &users, w);
    (roles.list, users, rows)
}

/// Кто сможет войти после переноса — пользователи с перенесённым паролем (если таких нет, программа
/// остаётся без входа, и считаются все). Если права «Пользователи и права» нет ни у одной их роли,
/// оно (с просмотром журнала) выдаётся роли первого из них: иначе управлять пользователями смогли бы
/// только те, кто войти не может, а у программы для компьютера нет `create-admin`.
fn grant_users_manage(roles: &mut Roles, users: &[ImportedUser], w: &mut Warnings) {
    let with_password: Vec<&ImportedUser> =
        users.iter().filter(|u| !u.legacy_sha1.is_empty()).collect();
    let candidates: Vec<&ImportedUser> = if with_password.is_empty() {
        users.iter().collect()
    } else {
        with_password
    };
    let Some(first) = candidates.first() else {
        return;
    };
    let manages = |u: &ImportedUser| {
        roles
            .list
            .iter()
            .any(|r| r.name == u.role && r.permissions.contains(Permission::UsersManage))
    };
    if candidates.iter().any(|u| manages(u)) {
        return;
    }
    // Право было, но только у тех, чей пароль не перенёсся.
    let only_without_password = users.iter().any(manages);
    if let Some(role) = roles.list.iter_mut().find(|r| r.name == first.role) {
        role.permissions.insert(Permission::UsersManage);
        role.permissions.insert(Permission::AuditView);
        let why = if only_without_password {
            format!(
                "Право «Пользователи и права» было только у пользователей, чей пароль не перенёсся (войти они не смогут, пока им не зададут новый): оно выдано роли «{}» первого пользователя с перенесённым паролем.",
                role.name
            )
        } else {
            format!(
                "Ни у одной роли пользователей не было права «Пользователи и права»: оно выдано роли «{}».",
                role.name
            )
        };
        w.push(
            WarningCode::UsersManageGranted,
            "Users",
            &role.name,
            "",
            "",
            &format!(
                "{why} Если войти не удаётся, создайте администратора командой mw-server create-admin (если это программа для компьютера — обратитесь к тому, кто её устанавливал)."
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::FixtureVersion::{V31, V32};
    use crate::report::Warnings;
    use crate::test_support::{codes, conversion_of, prepared};
    use mw_core::legacy_sha1_hex;
    use mw_domain::permission::{Permission, PermissionSet};

    #[tokio::test]
    async fn access_maps_groups_logins_and_hashes() {
        let (db, _) = prepared(V32).await;
        let mut w = Warnings::default();
        let (roles, users, rows) = convert_access(&db, &mut w);
        assert_eq!(
            roles.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
            [
                "Администратор",
                "Админы",
                "Мастера",
                "Администратор (MyWork 2)",
                "Без прав"
            ]
        );
        assert_eq!(roles[0].permissions, PermissionSet::all());
        assert_eq!(roles[1].permissions, PermissionSet::all());
        assert_eq!(
            roles[2].permissions,
            PermissionSet::from_codes(["orders_edit", "clients_view", "clients_edit"])
        );
        assert_eq!(
            users
                .iter()
                .map(|u| (u.login.as_str(), u.role.as_str()))
                .collect::<Vec<_>>(),
            [
                ("Ирина", "Админы"),
                ("ирина (2)", "Мастера"),
                ("Пётр", "Без прав"),
                ("Олег", "Мастера")
            ]
        );
        assert_eq!(users[2].legacy_sha1, legacy_sha1_hex("qwerty"));
        assert_eq!(rows.iter().filter(|r| r.source == "Users").count(), 4);
        for pair in [
            ("login_renamed", "2"),
            ("user_group_missing", "3"),
            ("user_password_invalid", "4"),
        ] {
            assert!(codes(&w).contains(&pair), "{pair:?}");
        }
        let dump = format!("{rows:?} {:?}", w.list());
        assert!(!dump.contains(&legacy_sha1_hex("1234")));
        assert!(!dump.contains(&legacy_sha1_hex("qwerty")));
        assert!(!dump.contains("zzz"));

        // V31: единственный пользователь в группе без флага settings
        let (db31, _) = prepared(V31).await;
        let mut w31 = Warnings::default();
        let (roles31, users31, _) = convert_access(&db31, &mut w31);
        let role = roles31.iter().find(|r| r.name == users31[0].role).unwrap();
        assert!(
            role.permissions.contains(Permission::UsersManage)
                && role.permissions.contains(Permission::AuditView)
        );
        assert_eq!(codes(&w31), [("users_manage_granted", "Мастера")]);
        // Выход — и для сервера, и для программы на компьютере, где команды под рукой нет.
        let message = &w31.list()[0].message;
        assert!(message.contains("mw-server create-admin"), "{message}");
        assert!(message.contains("программ"), "{message}");
        assert!(message.contains("устанавливал"), "{message}");
    }

    /// Пароль администраторов MyWork 2 не перенёсся, а у других пользователей — перенёсся: войти
    /// смогут только они, и право «Пользователи и права» получает роль первого из них.
    #[tokio::test]
    async fn users_manage_goes_to_someone_who_can_sign_in() {
        use crate::test_support::conversion_with_sql;
        let granted = |c: &crate::convert::Conversion| {
            c.warnings
                .list()
                .iter()
                .filter(|w| w.code.as_str() == "users_manage_granted")
                .map(|w| (w.key.clone(), w.message.clone()))
                .collect::<Vec<_>>()
        };
        // Ирина — единственная в «Админы» (все права); её пароль не похож на SHA-1. Первый с
        // перенесённым паролем — «ирина (2)» в «Мастера».
        let c = conversion_with_sql(V32, &["UPDATE Users SET user_pwd = 'нет' WHERE id = 1"]).await;
        let role = |name: &str| c.bundle.roles.iter().find(|r| r.name == name).unwrap();
        let masters = &role("Мастера").permissions;
        assert!(
            masters.contains(Permission::UsersManage) && masters.contains(Permission::AuditView),
            "{masters:?}"
        );
        let list = granted(&c);
        assert_eq!(list.len(), 1, "{list:?}");
        assert_eq!(list[0].0, "Мастера");
        let message = &list[0].1;
        assert!(message.contains("пароль не перенёсся"), "{message}");
        assert!(message.contains("«Мастера»"), "{message}");
        assert!(!message.contains("Ни у одной роли"), "{message}");
        assert!(message.contains("mw-server create-admin"), "{message}");

        // Не перенёсся ни один пароль: программа после переноса — без входа, искать некого.
        let none = conversion_with_sql(V32, &["UPDATE Users SET user_pwd = ''"]).await;
        assert!(granted(&none).is_empty());
        // Пароль администратора перенёсся — ничего не выдаётся.
        assert!(granted(&conversion_of(V32).await).is_empty());
    }
}
