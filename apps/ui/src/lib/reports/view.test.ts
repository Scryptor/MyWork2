import { describe, expect, it } from 'vitest';
import { reportQuery, reportViewFromSearch, reportViewToSearch, type ReportView } from './view.js';

const fromSearch = (s: string) => reportViewFromSearch(new URLSearchParams(s));
const plain: ReportView = {
	tab: 'orders',
	period: 'month',
	from: null,
	to: null,
	groupBy: 'master'
};

describe('report view in the address', () => {
	it('the default view is an empty address', () => {
		expect(reportViewToSearch(plain)).toBe('');
		expect(fromSearch('')).toEqual(plain);
	});

	it('goes there and back', () => {
		const search = 'period=custom&from=2026-11-01&to=2026-11-30&by=source&p=3&b=Apple';
		const view = fromSearch(search);
		expect(view).toEqual({
			...plain,
			period: 'custom',
			from: '2026-11-01',
			to: '2026-11-30',
			groupBy: 'source',
			servicePointId: 3,
			brand: 'Apple'
		});
		expect(reportViewToSearch(view)).toBe(search);
	});

	it('every filter and the other tab survive the trip', () => {
		const search = 'tab=moves&period=all&by=type&m=7&p=2&t=%D0%A1%D0%BC%D0%B0%D1%80%D1%82&s=%D0%90';
		expect(reportViewToSearch(fromSearch(search))).toBe(search);
		expect(fromSearch(search).tab).toBe('moves');
	});

	it('«Свой период» without days stays custom with no days', () => {
		const view = fromSearch('?period=custom');
		expect(view).toEqual({ ...plain, period: 'custom' });
		expect(reportViewToSearch(view)).toBe('period=custom');
	});

	it('days of a quick period are not kept', () => {
		expect(fromSearch('period=week&from=2026-11-01&to=2026-11-30')).toEqual({
			...plain,
			period: 'week'
		});
	});

	it('wrong values fall back to the default', () => {
		expect(fromSearch('by=bad&m=-1&p=0&period=x&tab=orders')).toEqual(plain);
		expect(fromSearch('period=custom&from=2026-13-01&to=nope')).toEqual({
			...plain,
			period: 'custom'
		});
	});
});

describe('reportQuery', () => {
	it('a quick period becomes its days by the local today', () => {
		expect(reportQuery(plain, '2026-11-25')).toEqual({
			from: '2026-11-01',
			to: '2026-11-25',
			group_by: 'master'
		});
	});

	it('«Всё время» has no bounds, custom keeps its own days, the filters go as they are', () => {
		const all = reportQuery(
			{ ...plain, period: 'all', groupBy: 'service_point', masterUserId: 4, brand: 'Apple' },
			'2026-11-25'
		);
		expect(all).toEqual({ group_by: 'service_point', master_user_id: 4, brand: 'Apple' });
		expect('from' in all).toBe(false);
		expect(
			reportQuery({ ...plain, period: 'custom', from: '2026-01-01', to: null }, '2026-11-25')
		).toEqual({ from: '2026-01-01', group_by: 'master' });
	});
});
