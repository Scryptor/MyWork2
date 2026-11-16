import { readStorage, writeStorage } from '#lib/design/storage.js';
import { normalizeSearch } from '#lib/format/search.js';
import { parseSavedViews, type SavedView } from './view.js';

/** «Мои виды» of the orders list, kept on this device. */
export const SAVED_VIEWS_KEY = 'mw.views';

export function createSavedViews() {
	let list = $state.raw<SavedView[]>(parseSavedViews(readStorage(SAVED_VIEWS_KEY)));

	function commit(next: SavedView[]) {
		list = next;
		writeStorage(SAVED_VIEWS_KEY, JSON.stringify(next));
	}

	return {
		get list(): SavedView[] {
			return list;
		},
		/** A view under a name; the same name (in any case) is replaced in its place. */
		add(name: string, search: string): void {
			const clean = name.trim();
			if (clean === '') return;
			const key = normalizeSearch(clean);
			const at = list.findIndex((v) => normalizeSearch(v.name) === key);
			const view = { name: clean, search };
			commit(at < 0 ? [...list, view] : list.with(at, view));
		},
		remove(index: number): void {
			commit(list.filter((_, i) => i !== index));
		}
	};
}

export const savedViews = createSavedViews();
