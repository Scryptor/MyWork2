import { legacyStockState } from '#lib/api/import.js';

/**
 * Whether the stock of MyWork 2 kept by an import made before the stock module still waits to be transferred
 * (decision 17): while it does, «Склад» and «Добавить позицию» warn that the first position — even one deleted
 * later — closes the transfer. Asked when «Склад» opens and after a position is added; the newest answer wins.
 * Without an answer (no connection) the last one stays.
 */
export function createLegacyStock() {
	let pending = $state(false);
	let seq = 0;
	return {
		get pending(): boolean {
			return pending;
		},
		refresh(): void {
			const mine = ++seq;
			legacyStockState().then(
				(s) => {
					if (mine === seq) pending = s.available;
				},
				() => {}
			);
		}
	};
}
