import type { FolderCheck } from '$api/FolderCheck';
import type { ImportJob } from '$api/ImportJob';
import type { ImportReportView } from '$api/ImportReportView';
import type { LegacyImportCheck } from '$api/LegacyImportCheck';
import type { LegacyImportItem } from '$api/LegacyImportItem';
import type { LegacyImportStart } from '$api/LegacyImportStart';
import type { LegacyStockState } from '$api/LegacyStockState';
import type { StockImportSummary } from '$api/StockImportSummary';
import { request } from './client.js';

// The transfer from MyWork 2 (spec 6.8, 6.9): the path is on the disk of the server, so the check, the start
// and the progress are for a server on this computer only (403 elsewhere); the past imports are read anywhere.

/** What the folder holds, or why it cannot be transferred (nothing is written). */
export const checkLegacyFolder = (body: LegacyImportCheck) =>
	request<FolderCheck>('POST', '/legacy-import/check', { body });

/** Starts the transfer in the background (202); a second one while it runs is 409 «Импорт уже идёт». */
export const startLegacyImport = (body: LegacyImportStart) =>
	request<ImportJob>('POST', '/legacy-import', { body });

/**
 * The progress of the import of this server's run; null when there was none since it started. A 401 is
 * left to the caller: the base the import wrote may have users of its own (or the replacement closed every
 * session), and the screen asks to sign in as one of them.
 */
export const currentLegacyImport = () =>
	request<ImportJob | null>('GET', '/legacy-import/current', { quietUnauthenticated: true });

/** The past imports, the latest first. */
export const listLegacyImports = () => request<LegacyImportItem[]>('GET', '/legacy-imports');

/** The report of a past import. */
export const getLegacyImport = (id: number) =>
	request<ImportReportView>('GET', `/legacy-imports/${id}`);

/**
 * Can the stock of MyWork 2 that the last import kept as it was (an import made before the stock module) be
 * transferred now: it can while the base has no stock items and no parts of orders.
 */
export const legacyStockState = () => request<LegacyStockState>('GET', '/legacy-import/stock');

/**
 * Transfers that stock in one go (the folder of MyWork 2 is not needed): 422 `stock` `not_empty` when the
 * base already has stock (a repeat too), 409 while an import runs.
 */
export const migrateLegacyStock = () => request<StockImportSummary>('POST', '/legacy-import/stock');
