import { afterEach, describe, expect, it } from 'vitest';
import { SAVED_VIEWS_KEY, createSavedViews } from './saved-views.svelte.js';

afterEach(() => localStorage.clear());

describe('saved views', () => {
	it('are kept on this device under mw.views', () => {
		const views = createSavedViews();
		expect(SAVED_VIEWS_KEY).toBe('mw.views');
		expect(views.list).toEqual([]);
		views.add('  Готовые Samsung ', 'chips=3&brand=Samsung');
		views.add('Звонки', 'attention=call');
		expect(views.list).toEqual([
			{ name: 'Готовые Samsung', search: 'chips=3&brand=Samsung' },
			{ name: 'Звонки', search: 'attention=call' }
		]);
		expect(JSON.parse(localStorage.getItem('mw.views')!)).toEqual(views.list);
		expect(createSavedViews().list).toEqual(views.list);
	});

	it('the same name replaces the view in its place; remove drops one; a blank name is not saved', () => {
		const views = createSavedViews();
		views.add('Звонки', 'attention=call');
		views.add('Готовые', 'chips=3');
		views.add('звонки', 'attention=call&brand=Asus');
		expect(views.list).toEqual([
			{ name: 'звонки', search: 'attention=call&brand=Asus' },
			{ name: 'Готовые', search: 'chips=3' }
		]);
		views.add('   ', 'chips=all');
		expect(views.list).toHaveLength(2);
		views.remove(0);
		expect(views.list).toEqual([{ name: 'Готовые', search: 'chips=3' }]);
		expect(createSavedViews().list).toEqual(views.list);
	});
});
