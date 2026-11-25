import type { Download } from '@playwright/test';

/**
 * The name of a downloaded file as the server sent it (`filename*`, with spaces). WebKit on a Mac gives the
 * letters taken apart (NFD: «ё» as «е» and a mark); WebKit on Linux gives «_» for the spaces.
 */
export const downloadName = (download: Download): string =>
	download.suggestedFilename().normalize('NFC').replaceAll('_', ' ');
