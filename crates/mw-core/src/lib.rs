//! Прикладной слой MyWork2: бизнес-правила поверх `mw-db`. Все операции — методы `Core`.

mod actor;
pub mod attachments;
mod audit;
mod auth;
pub mod backups;
mod clients;
mod clients_merge;
mod clock;
mod core;
mod dictionaries;
mod dictionaries_rewrite;
mod document_templates;
pub mod documents;
mod error;
mod export;
mod gate;
mod import;
mod legacy_templates;
mod money_text;
mod orders;
mod page;
mod patch;
mod reports;
mod search_index;
mod service_points;
mod settings;
mod setup;
mod sql;
mod statuses;
mod stock;
mod usage;

pub use actor::Actor;
pub use attachments::{Attachment, AttachmentFile, AttachmentKind, AttachmentVariant, FirmLogo};
pub use audit::{AuditEntry, AuditFilter};
pub use auth::{
    LAST_SEEN_THROTTLE_SECS, MAX_CONCURRENT_PASSWORD_HASHES, MIN_PASSWORD_CHARS, MasterUser,
    NewRole, NewUser, PasswordScheme, Role, RolePatch, SESSION_IDLE_DAYS, Session, User, UserPatch,
    hash_password, legacy_sha1_hex, verify_password,
};
pub use backups::{
    BackupCopy, BackupJob, BackupKind, BackupLegacySettings, BackupOverview, BackupRestorePending,
    BackupRestoreResult, BackupSettings, BackupSettingsPatch,
};
pub use clients::{
    Client, ClientFilter, ClientListItem, ClientPatch, ClientPhone, ClientSort, ClientSortField,
    MAX_CLIENT_PHONES, NewClient,
};
pub use clients_merge::{MAX_MERGE_SOURCES, MAX_SIMILAR_CLIENTS, MergeClients, VersionedId};
pub use clock::{Clock, FixedClock, SystemClock};
pub use core::Core;
pub use dictionaries::{
    DictionaryItem, DictionaryItemPatch, DictionaryMerge, DictionaryRename, RewriteResult,
};
pub use document_templates::{
    DocumentField, DocumentTemplate, DocumentTemplateInfo, TemplatePreset, TemplateSave,
};
pub use error::CoreError;
pub use export::contacts::ContactsFormat;
pub use export::{EXPORT_ROWS_MAX, ExportFile};
pub use import::{
    DatabaseKind, ExtraCostsChange, ImportBundle, ImportMode, ImportSummary, ImportTarget,
    ImportedClient, ImportedHistory, ImportedLegacyRow, ImportedOrder, ImportedOrderPart,
    ImportedSettings, ImportedStock, ImportedStockItem, ImportedUser, LegacyImportMode,
    LegacyImportRecord, LegacyOrderCosts, LegacySourceRow, LegacyStockSource, LegacyStockState,
    LegacyTarget, StockImport, StockImportSummary,
};
pub use legacy_templates::LegacyTemplate;
pub use orders::{
    ClientRef, IssuedFilter, NewOrder, NewOrderComment, NewWarrantyCase, Order, OrderComment,
    OrderFilter, OrderListItem, OrderPatch, OrderSort, OrderSortField, OrderSummary,
    OrderValueField, StatusCount,
};
pub use page::{Page, PageRequest};
pub use reports::{
    MoveItem, MoveRoute, MovesFilter, MovesReport, OrderReport, OrderReportGroup, ReportFilter,
    ReportGroupBy, ReportPoint, ReportStep, ReportTotals,
};
pub use search_index::{ReindexStats, SEARCH_INDEX_VERSION};
pub use service_points::{ServicePoint, ServicePointPatch};
pub use settings::{AppSettings, SettingsPatch};
pub use setup::SetupRequest;
pub use statuses::{NewStatus, Status, StatusPatch};
pub use stock::{
    NewOrderPart, NewStockItem, OrderPart, OrderParts, StockFilter, StockItem, StockItemPatch,
    StockSort, StockSortField, StockSummary,
};
pub use usage::CatalogUsage;

pub(crate) use audit::{EditVia, NewAuditEntry, write_audit};
