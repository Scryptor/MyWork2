import { errorReason } from '#lib/api/client.js';
import { ru } from '#lib/i18n/ru.js';
import { toast } from '#lib/ui/toast/toast.svelte.js';

/** Runs a download; a failure is a toast with the server's own words (too many rows, no right). */
export async function runExport(download: () => Promise<void>): Promise<void> {
	try {
		await download();
	} catch (e) {
		toast.show({
			message: `${ru.exports.failed}: ${errorReason(e)}`,
			tone: 'danger'
		});
	}
}
