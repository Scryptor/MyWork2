import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import PhotoStrip from './PhotoStrip.svelte';

const photos = [1, 2, 3].map((i) => ({
	id: `p${i}`,
	src: `data:image/gif;base64,R0lGODlhAQABAAAAACw=`,
	alt: `Фото ${i}`
}));

describe('PhotoStrip', () => {
	it('shows the thumbnails, the counter «3 из 10» and the «＋» tile', async () => {
		const onadd = vi.fn();
		render(PhotoStrip, { photos, onadd });
		const strip = screen.getByRole('group', { name: 'Фото устройства' });
		expect(
			within(strip)
				.getAllByRole('img')
				.map((i) => i.getAttribute('alt'))
		).toEqual(['Фото 1', 'Фото 2', 'Фото 3']);
		expect(strip).toHaveTextContent('3 из 10');
		await userEvent.click(within(strip).getByRole('button', { name: 'Добавить фото' }));
		expect(onadd).toHaveBeenCalledOnce();
	});

	it('remove and open report the photo id', async () => {
		const onremove = vi.fn();
		const onopen = vi.fn();
		render(PhotoStrip, { photos, onremove, onopen });
		await userEvent.click(screen.getByRole('button', { name: 'Удалить фото: Фото 2' }));
		expect(onremove).toHaveBeenCalledWith('p2');
		await userEvent.click(screen.getByRole('button', { name: 'Фото 3' }));
		expect(onopen).toHaveBeenCalledWith('p3');
	});

	it('at the maximum there is no «＋» tile, nor without onadd', () => {
		const { unmount } = render(PhotoStrip, { photos, max: 3, label: 'Фото', onadd: vi.fn() });
		const strip = screen.getByRole('group', { name: 'Фото' });
		expect(strip).toHaveTextContent('3 из 3');
		expect(within(strip).queryByRole('button', { name: 'Добавить фото' })).toBeNull();
		unmount();

		render(PhotoStrip, { photos: [] });
		expect(screen.getByRole('group', { name: 'Фото устройства' })).toHaveTextContent('0 из 10');
		expect(screen.queryByRole('button', { name: 'Добавить фото' })).toBeNull();
	});

	it('without onopen and onremove the thumbnails are plain pictures', () => {
		render(PhotoStrip, { photos });
		expect(screen.queryAllByRole('button')).toEqual([]);
		expect(screen.getAllByRole('img')).toHaveLength(3);
	});
});
