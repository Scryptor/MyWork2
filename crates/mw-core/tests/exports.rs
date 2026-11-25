//! Выгрузки заказов и клиентов в Excel: строки и порядок как в списках, колонки, права.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use std::io::Cursor;

use calamine::{Data, Reader, Xlsx, open_workbook_from_rs};
use common::*;
use mw_core::{
    ClientFilter, ContactsFormat, CoreError, NewClient, NewOrder, OrderFilter, OrderPatch,
    OrderSort,
};
use mw_domain::money::Money;
use mw_domain::permission::Permission;

fn rub(r: i64) -> Money {
    Money::from_rubles(r)
}

fn sheet(bytes: Vec<u8>, name: &str) -> Vec<Vec<String>> {
    let mut book: Xlsx<Cursor<Vec<u8>>> = open_workbook_from_rs(Cursor::new(bytes)).unwrap();
    book.worksheet_range(name)
        .unwrap()
        .rows()
        .map(|r| {
            r.iter()
                .map(|c| match c {
                    Data::Empty => String::new(),
                    Data::String(s) => s.clone(),
                    Data::Float(f) => f.to_string(),
                    Data::Int(i) => i.to_string(),
                    other => other.to_string(),
                })
                .collect()
        })
        .collect()
}

const ORDER_COLUMNS: [&str; 24] = [
    "№ заказа",
    "Принят",
    "Статус",
    "Выдан",
    "Точка",
    "Клиент",
    "Телефон",
    "Тип устройства",
    "Бренд",
    "Модель",
    "Серийный номер",
    "Неисправность",
    "Выполненные работы",
    "Мастер",
    "Оценка от",
    "Оценка до",
    "Итог",
    "Скидка",
    "Предоплата",
    "Расходы",
    "Запчасти",
    "К оплате",
    "Гарантия",
    "Штрихкод",
];

#[tokio::test]
async fn orders_export_follows_the_list_filter() -> Result<(), CoreError> {
    for t in cores().await {
        let brand = "Выгрузка-Бренд";
        let mut made = Vec::new();
        for i in 0..3 {
            let o = t
                .core
                .create_order(
                    &admin(),
                    NewOrder {
                        brand: brand.into(),
                        ..sample_new_order()
                    },
                )
                .await?;
            let patch = OrderPatch {
                device_password: (i == 0).then(|| "1234".to_owned()),
                device_pattern: (i == 0).then(|| "1235".to_owned()),
                final_price: (i != 2).then_some(Some(rub(1000))),
                prepayment: (i != 2).then_some(rub(300)),
                discount: (i != 2).then_some(rub(100)),
                serial: Some(format!("SN-{i}")),
                ..Default::default()
            };
            made.push(
                t.core
                    .update_order(&admin(), o.id, o.version, patch)
                    .await?,
            );
        }
        // Чужой заказ с другим брендом в выгрузку не входит.
        t.core.create_order(&admin(), sample_new_order()).await?;

        let filter = OrderFilter {
            brand: Some(brand.into()),
            ..Default::default()
        };
        let sort = OrderSort::default();
        let listed = t.core.list_orders(&admin(), &filter, sort, 50, 0).await?;
        let file = t.core.export_orders_xlsx(&admin(), &filter, sort).await?;
        assert_eq!(file.ascii_name, "orders.xlsx");
        assert!(file.name.starts_with("Заказы ") && file.name.ends_with(".xlsx"));
        let rows = sheet(file.bytes, "Заказы");
        assert_eq!(rows[0], ORDER_COLUMNS, "{}", t.label);
        assert_eq!(rows.len(), 4, "{}", t.label);
        let exported: Vec<String> = rows[1..].iter().map(|r| r[0].clone()).collect();
        let expected: Vec<String> = listed.items.iter().map(|i| i.id.to_string()).collect();
        assert_eq!(exported, expected, "{}", t.label);

        for row in &rows[1..] {
            let id: i64 = row[0].parse().unwrap();
            let col = |name: &str| {
                let at = ORDER_COLUMNS.iter().position(|c| *c == name).unwrap();
                row[at].clone()
            };
            assert_eq!(col("Бренд"), brand);
            assert_eq!(col("Телефон"), "+7 921 123-45-67");
            assert_eq!(col("Гарантия"), "30 дней");
            if id == made[2].id {
                assert_eq!(col("К оплате"), "", "без итога — пусто");
                assert_eq!(col("Итог"), "");
            } else {
                assert_eq!(col("Итог"), "1000");
                assert_eq!(col("Предоплата"), "300");
                assert_eq!(col("К оплате"), "600", "итог − предоплата − скидка");
            }
        }
        assert!(
            !rows
                .iter()
                .flatten()
                .any(|c| c.contains("1234") || c.contains("1235")),
            "пароль и графический ключ не выгружаются"
        );
        assert!(
            !rows[0]
                .iter()
                .any(|c| c.contains("Пароль") || c.contains("Графический")),
        );
    }
    Ok(())
}

#[tokio::test]
async fn clients_export_follows_the_search() -> Result<(), CoreError> {
    for t in cores().await {
        t.core
            .create_client(
                &admin(),
                NewClient {
                    full_name: "Выгрузкин Уникум".into(),
                    phones: phones(&["+7 921 123-45-67", "89217654321"]),
                    address: "ул. Тестовая".into(),
                    source: "Сайт".into(),
                    blacklisted: true,
                    note: "=заметка".into(),
                },
            )
            .await?;
        make_client(&t, "Другой Человек", &["+7 900 000-00-01"]).await;
        let filter = ClientFilter {
            query: "уникум".into(),
            ..Default::default()
        };
        let file = t.core.export_clients_xlsx(&admin(), &filter).await?;
        assert_eq!(file.ascii_name, "clients.xlsx");
        assert!(file.name.starts_with("Клиенты ") && file.name.ends_with(".xlsx"));
        let rows = sheet(file.bytes, "Клиенты");
        assert_eq!(
            rows[0],
            [
                "№ клиента",
                "ФИО",
                "Телефоны",
                "Адрес",
                "Откуда узнал",
                "Чёрный список",
                "Заказов",
                "Примечание",
                "Добавлен"
            ],
            "{}",
            t.label
        );
        assert_eq!(rows.len(), 2, "{}", t.label);
        assert_eq!(rows[1][1], "Выгрузкин Уникум");
        assert_eq!(rows[1][2], "+7 921 123-45-67, +7 921 765-43-21");
        assert_eq!(rows[1][3], "ул. Тестовая");
        assert_eq!(rows[1][4], "Сайт");
        assert_eq!(rows[1][5], "да");
        assert_eq!(rows[1][6], "0");
        assert_eq!(rows[1][7], "=заметка");

        // Без поиска — все клиенты в порядке списка.
        let all = ClientFilter::default();
        let listed = t.core.list_clients(&admin(), &all, 50, 0).await?;
        let rows = sheet(
            t.core.export_clients_xlsx(&admin(), &all).await?.bytes,
            "Клиенты",
        );
        let exported: Vec<String> = rows[1..].iter().map(|r| r[0].clone()).collect();
        let expected: Vec<String> = listed.items.iter().map(|i| i.id.to_string()).collect();
        assert_eq!(exported, expected, "{}", t.label);
    }
    Ok(())
}

#[tokio::test]
async fn exports_need_data_export() -> Result<(), CoreError> {
    for t in cores().await {
        let no_export = actor_with(&[Permission::ClientsView, Permission::OrdersEdit]);
        assert!(matches!(
            t.core
                .export_orders_xlsx(&no_export, &OrderFilter::default(), OrderSort::default())
                .await,
            Err(CoreError::Forbidden(Permission::DataExport))
        ));
        assert!(matches!(
            t.core
                .export_clients_xlsx(&no_export, &ClientFilter::default())
                .await,
            Err(CoreError::Forbidden(Permission::DataExport))
        ));
        let no_clients = actor_with(&[Permission::DataExport]);
        assert!(matches!(
            t.core
                .export_clients_xlsx(&no_clients, &ClientFilter::default())
                .await,
            Err(CoreError::Forbidden(Permission::ClientsView))
        ));
        t.core
            .export_orders_xlsx(&no_clients, &OrderFilter::default(), OrderSort::default())
            .await?;
    }
    Ok(())
}

fn text(file: mw_core::ExportFile) -> String {
    String::from_utf8(file.bytes).unwrap()
}

#[tokio::test]
async fn a_phone_goes_to_the_first_client_only() -> Result<(), CoreError> {
    for t in cores().await {
        make_client(&t, "Альфа Телефонов", &["8 (921) 111-22-33", "12"]).await;
        make_client(&t, "Бета Телефонов", &["+7 921 111 22 33"]).await;
        make_client(&t, "Гамма Телефонов", &["+7 921 111 22 33", "123-45-67"]).await;
        let filter = ClientFilter {
            query: "телефонов".into(),
            sort: mw_core::ClientSort {
                field: mw_core::ClientSortField::Name,
                descending: false,
            },
            ..Default::default()
        };
        let file = t
            .core
            .export_contacts(&admin(), &filter, None, ContactsFormat::Csv)
            .await?;
        assert_eq!(file.ascii_name, "contacts.csv");
        assert_eq!(file.content_type, "text/csv; charset=utf-8");
        assert!(file.name.starts_with("Контакты ") && file.name.ends_with(".csv"));
        let csv = text(file);
        assert!(
            csv.contains("Альфа Телефонов,Альфа Телефонов,* myContacts,Mobile,+79211112233\r\n")
        );
        assert!(!csv.contains("Бета"), "{}: {csv}", t.label);
        assert!(csv.contains("Гамма Телефонов,Гамма Телефонов,* myContacts,Mobile,1234567\r\n"));
        assert_eq!(csv.matches("+79211112233").count(), 1);

        let card = text(
            t.core
                .export_contacts(&admin(), &filter, None, ContactsFormat::Vcard)
                .await?,
        );
        assert!(card.starts_with("BEGIN:VCARD\r\n"));
        assert_eq!(card.matches("BEGIN:VCARD").count(), 2);
    }
    Ok(())
}

#[tokio::test]
async fn contacts_since_a_day() -> Result<(), CoreError> {
    for t in cores().await {
        make_client(&t, "Вчерашний Контакт", &["+7 900 000-00-11"]).await;
        t.clock.advance(chrono::Duration::days(1));
        make_client(&t, "Сегодняшний Контакт", &["+7 900 000-00-22"]).await;
        let today = t.core.now().date();
        let csv = text(
            t.core
                .export_contacts(
                    &admin(),
                    &ClientFilter::default(),
                    Some(today),
                    ContactsFormat::Csv,
                )
                .await?,
        );
        assert!(csv.contains("Сегодняшний"), "{}", t.label);
        assert!(!csv.contains("Вчерашний"), "{}", t.label);
        let all = text(
            t.core
                .export_contacts(
                    &admin(),
                    &ClientFilter::default(),
                    None,
                    ContactsFormat::Csv,
                )
                .await?,
        );
        assert!(all.contains("Вчерашний") && all.contains("Сегодняшний"));
    }
    Ok(())
}

#[tokio::test]
async fn no_contacts_is_an_error() -> Result<(), CoreError> {
    for t in cores().await {
        make_client(&t, "Без Телефона", &[]).await;
        make_client(&t, "Короткий Номер", &["12"]).await;
        for format in [ContactsFormat::Csv, ContactsFormat::Vcard] {
            assert_validation(
                t.core
                    .export_contacts(&admin(), &ClientFilter::default(), None, format)
                    .await,
                &[("rows", "nothing_to_export")],
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn contacts_need_data_export() -> Result<(), CoreError> {
    for t in cores().await {
        let no_export = actor_with(&[Permission::ClientsView]);
        assert!(matches!(
            t.core
                .export_contacts(
                    &no_export,
                    &ClientFilter::default(),
                    None,
                    ContactsFormat::Csv
                )
                .await,
            Err(CoreError::Forbidden(Permission::DataExport))
        ));
        let no_clients = actor_with(&[Permission::DataExport]);
        assert!(matches!(
            t.core
                .export_contacts(
                    &no_clients,
                    &ClientFilter::default(),
                    None,
                    ContactsFormat::Vcard
                )
                .await,
            Err(CoreError::Forbidden(Permission::ClientsView))
        ));
    }
    Ok(())
}
