import type { HandleClientError } from '@sveltejs/kit/hooks';
import { ru } from '#lib/i18n/ru.js';

// Errors the program throws with error() keep their text; SvelteKit's own and unknown ones get a
// Russian one instead of «Not Found» / «Internal Error».
export const handleError: HandleClientError = ({ kind, error }) => {
	if (kind === 'app') return;
	if (kind === 'framework' && error.status === 404) return { message: ru.app.notFound };
	if (kind === 'unknown') console.error(error);
	return { message: ru.app.unexpected };
};
