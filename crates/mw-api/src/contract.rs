//! Выгрузка TypeScript-типов API.

use std::path::Path;

use ts_rs::TS;

use crate::{
    AboutInfo, AttachmentNameQuery, AuditListQuery, AuditPage, BarcodeQuery, CaptionPatch,
    ChangePasswordRequest, ClientListQuery, ClientPage, CurrentUser, DictionaryListQuery,
    DictionaryValue, ErrorBody, IncludeArchived, LoginRequest, LoginResponse, NewServicePoint,
    NextOrderNumber, OrderListQuery, OrderPage, OrderSummaryQuery, OrderValuesQuery, PageQuery,
    PhoneQuery, ReorderRequest, ServerInfo, SetPasswordRequest, StatusChange, VersionParam,
};

/// Выгружает в `dir` TS-типы всех корневых типов API вместе с зависимостями
/// (по файлу на тип). `i64` выгружается как `number`: суммы и id меньше 2^53.
///
/// `dir` должна быть новой пустой папкой: ts-rs дописывает в существующие файлы.
pub fn export_typescript(dir: &Path) -> Result<(), ts_rs::ExportError> {
    let cfg = ts_rs::Config::new()
        .with_large_int("number")
        .with_out_dir(dir);
    // Единый список корневых типов: каждая следующая задача добавляет сюда
    // свои типы тел, ответов и параметров запросов.
    ErrorBody::export_all(&cfg)?;
    ServerInfo::export_all(&cfg)?;
    LoginRequest::export_all(&cfg)?;
    LoginResponse::export_all(&cfg)?;
    CurrentUser::export_all(&cfg)?;
    ChangePasswordRequest::export_all(&cfg)?;
    IncludeArchived::export_all(&cfg)?;
    ReorderRequest::export_all(&cfg)?;
    DictionaryValue::export_all(&cfg)?;
    DictionaryListQuery::export_all(&cfg)?;
    AboutInfo::export_all(&cfg)?;
    mw_core::DictionaryItemPatch::export_all(&cfg)?;
    mw_core::CatalogUsage::export_all(&cfg)?;
    NewServicePoint::export_all(&cfg)?;
    ClientListQuery::export_all(&cfg)?;
    PhoneQuery::export_all(&cfg)?;
    AttachmentNameQuery::export_all(&cfg)?;
    CaptionPatch::export_all(&cfg)?;
    mw_core::Attachment::export_all(&cfg)?;
    VersionParam::export_all(&cfg)?;
    ClientPage::export_all(&cfg)?;
    OrderListQuery::export_all(&cfg)?;
    BarcodeQuery::export_all(&cfg)?;
    StatusChange::export_all(&cfg)?;
    PageQuery::export_all(&cfg)?;
    mw_core::NewWarrantyCase::export_all(&cfg)?;
    NextOrderNumber::export_all(&cfg)?;
    OrderPage::export_all(&cfg)?;
    OrderSummaryQuery::export_all(&cfg)?;
    OrderValuesQuery::export_all(&cfg)?;
    mw_core::OrderSummary::export_all(&cfg)?;
    mw_core::NewOrder::export_all(&cfg)?;
    mw_core::OrderPatch::export_all(&cfg)?;
    mw_core::Order::export_all(&cfg)?;
    mw_core::Client::export_all(&cfg)?;
    mw_core::NewClient::export_all(&cfg)?;
    mw_core::ClientPatch::export_all(&cfg)?;
    mw_core::MergeClients::export_all(&cfg)?;
    SetPasswordRequest::export_all(&cfg)?;
    AuditListQuery::export_all(&cfg)?;
    AuditPage::export_all(&cfg)?;
    mw_core::NewUser::export_all(&cfg)?;
    mw_core::UserPatch::export_all(&cfg)?;
    mw_core::User::export_all(&cfg)?;
    mw_core::NewRole::export_all(&cfg)?;
    mw_core::RolePatch::export_all(&cfg)?;
    mw_core::Role::export_all(&cfg)?;
    mw_core::AppSettings::export_all(&cfg)?;
    mw_core::SettingsPatch::export_all(&cfg)?;
    mw_core::Status::export_all(&cfg)?;
    mw_core::NewStatus::export_all(&cfg)?;
    mw_core::StatusPatch::export_all(&cfg)?;
    mw_core::DictionaryItem::export_all(&cfg)?;
    mw_core::ServicePoint::export_all(&cfg)?;
    mw_core::ServicePointPatch::export_all(&cfg)?;
    mw_domain::dictionary::DictionaryKind::export_all(&cfg)?;
    Ok(())
}
