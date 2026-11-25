import type { BackupJob } from '$api/BackupJob';
import type { BackupOverview } from '$api/BackupOverview';
import type { BackupRestorePending } from '$api/BackupRestorePending';
import type { BackupSettings } from '$api/BackupSettings';
import type { BackupSettingsPatch } from '$api/BackupSettingsPatch';
import { request } from './client.js';

// «Настройки → Резервные копии»: the folder of copies is on the disk of the server, so every call is for a
// server on this computer only (403 elsewhere). Settings, copying and the choice about MyWork 2 need
// «settings_manage»; restoring and deleting a copy need «users_manage» too.

/** Everything for the screen: settings, the copies (the newest first), the running copy, the restore. */
export const getBackups = () => request<BackupOverview>('GET', '/backups');

/** Saves the given fields of the settings; a field not given stays. */
export const updateBackupSettings = (body: BackupSettingsPatch) =>
	request<BackupSettings>('PATCH', '/backups/settings', { body });

/** Starts a manual copy in the background (202); its progress is `job` of the overview. */
export const startBackup = () => request<BackupJob>('POST', '/backups');

export const deleteBackup = (name: string) =>
	request<void>('DELETE', `/backups/${encodeURIComponent(name)}`);

/** Asks to replace the data with the copy at the next start of the program (202). */
export const requestRestore = (name: string) =>
	request<BackupRestorePending>('POST', '/backups/restore', { body: { name } });

/** Takes the request back, or hides the result of the last restore. */
export const cancelRestore = () => request<void>('DELETE', '/backups/restore');

/** The answer to «Взять настройки MyWork 2»: `true` takes them, `false` leaves the own ones. */
export const chooseLegacyBackup = (apply: boolean) =>
	request<BackupSettings>('POST', '/backups/legacy', { body: { apply } });
