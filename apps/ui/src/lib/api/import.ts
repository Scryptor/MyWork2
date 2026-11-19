import type { FolderCheck } from '$api/FolderCheck';
import type { ImportJob } from '$api/ImportJob';
import type { ImportReportView } from '$api/ImportReportView';
import type { LegacyImportCheck } from '$api/LegacyImportCheck';
import type { LegacyImportItem } from '$api/LegacyImportItem';
import type { LegacyImportStart } from '$api/LegacyImportStart';
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
