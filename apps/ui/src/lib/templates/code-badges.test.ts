import { describe, expect, it } from 'vitest';
import type { FirmLink } from '$api/FirmLink';
import { badgeText, linkBadges, staticBadges } from './code-badges.js';

const link = (id: string, name: string, enabled = true): FirmLink => ({
	id,
	name,
	url: 'https://example.test',
	enabled
});

describe('code badges', () => {
	it('staticBadges are the nine of the table, in order, with their labels', () => {
		expect(staticBadges().map((b) => [b.id, b.label])).toEqual([
			['order_number', 'Номер заказа'],
			['client_name', 'ФИО клиента'],
			['client_phone', 'Телефон клиента'],
			['device', 'Устройство'],
			['received', 'Дата приёма'],
			['logo', 'Логотип'],
			['space', 'Отступ'],
			['client_copy', 'Только в экземпляре клиента'],
			['service_copy', 'Только в экземпляре сервиса']
		]);
	});

	it('linkBadges skips the disabled links and makes a QR badge of each of the others', () => {
		const badges = linkBadges([
			link('2gis', '2ГИС'),
			link('off', 'Выкл', false),
			link('tg', 'Telegram')
		]);
		expect(badges.map((b) => b.label)).toEqual(['QR-код 2ГИС', 'QR-код Telegram']);
		expect(badges[0]).toEqual({
			id: 'link.2gis',
			label: 'QR-код 2ГИС',
			markup: '#qr-image("link.2gis", 20mm)',
			code: 'qr-image("link.2gis", 20mm)'
		});
	});

	it('badgeText: code mode gets the code, markup gets the markup', () => {
		const b = staticBadges()[0]!;
		expect(badgeText(b, '#{ ')).toBe(b.code);
		expect(badgeText(b, 'Текст ')).toBe(b.markup);
		expect(b.markup).toBe('#v("order.number")');
	});
});
