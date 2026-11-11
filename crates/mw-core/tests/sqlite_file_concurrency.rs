//! Боевая конфигурация SQLite: файл, WAL, пул из 5 соединений, много одновременных писателей.
mod common;

use std::sync::Arc;

use chrono::{TimeZone, Utc};
use common::*;
use mw_core::{Core, FixedClock, OrderFilter, OrderPatch, OrderSort};
use sea_orm::EntityTrait;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_orders_on_file_sqlite_do_not_fail() {
    let dir = std::env::temp_dir().join(format!(
        "mw-core-conc-{}-{}",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("mw.sqlite");
    let db = mw_db::connect(&mw_db::sqlite_url(&path)).await.unwrap();
    mw_db::migrate(&db).await.unwrap();
    let clock = Arc::new(FixedClock::new(
        Utc.with_ymd_and_hms(2026, 11, 11, 9, 0, 0).unwrap(),
    ));
    let core = Core::open(db, clock, "UTC").await.unwrap();

    const TASKS: usize = 20;
    let mut handles = Vec::new();
    for i in 0..TASKS {
        let core = core.clone();
        handles.push(tokio::spawn(async move {
            let who = admin();
            let order = core.create_order(&who, sample_new_order()).await?;
            core.update_order(
                &who,
                order.id,
                1,
                OrderPatch {
                    master: Some(format!("мастер {i}")),
                    ..Default::default()
                },
            )
            .await?;
            core.list_orders(&who, &OrderFilter::default(), OrderSort::default(), 50, 0)
                .await?;
            Ok::<_, mw_core::CoreError>(())
        }));
    }
    for h in handles {
        h.await.unwrap().unwrap();
    }
    let total = core
        .list_orders(
            &admin(),
            &OrderFilter::default(),
            OrderSort::default(),
            1,
            0,
        )
        .await
        .unwrap()
        .total;
    assert_eq!(total, TASKS as u64);
    let rows = mw_db::entity::order::Entity::find()
        .all(core.db())
        .await
        .unwrap();
    assert!(rows.iter().all(|o| o.version == 2));

    core.db().clone().close().await.unwrap();
    std::fs::remove_dir_all(&dir).ok();
}
