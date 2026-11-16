// Fictional demo data for the gallery and the demo screens. Every name, phone and number here is made
// up; phones start with 7000 («+7 000 …»), names are in capitals as in MyWork 2 databases.
import type { OrderListItem } from '$api/OrderListItem';
import type { Status } from '$api/Status';
import type { StatusKind } from '$api/StatusKind';
import type { LocalDateTime } from '#lib/format/date.js';

function status(id: number, name: string, kind: StatusKind, color: string): Status {
	return { id, name, kind, color, sort_order: id - 1, archived: false };
}

/** The eight default statuses of a new database (names, kinds and colours as in mw-domain) and «Куплен». */
export const DEMO_STATUSES: Status[] = [
	status(1, 'Диагностика', 'diagnostics', 'amber'),
	status(2, 'Согласование с клиентом', 'open', 'violet'),
	status(3, 'Согласовано', 'open', 'indigo'),
	status(4, 'Принят в работу', 'open', 'blue'),
	status(5, 'Ждёт запчасть', 'open', 'red'),
	status(6, 'Принят по гарантии', 'open', 'teal'),
	status(7, 'Готов', 'ready', 'green'),
	status(8, 'Выдан', 'issued', 'gray'),
	status(9, 'Куплен', 'issued', 'gray')
];

export const DEMO_POINTS: { id: number; name: string }[] = [
	{ id: 1, name: 'Центр' },
	{ id: 2, name: 'Северная' },
	{ id: 3, name: 'Южная' }
];

const SURNAMES = [
	'ИВАНОВ',
	'ПЕТРОВ',
	'СИДОРОВ',
	'СМИРНОВ',
	'КУЗНЕЦОВ',
	'ПОПОВ',
	'ВАСИЛЬЕВ',
	'СОКОЛОВ',
	'МИХАЙЛОВ',
	'НОВИКОВ',
	'ФЁДОРОВ',
	'МОРОЗОВ',
	'ВОЛКОВ',
	'АЛЕКСЕЕВ',
	'ЛЕБЕДЕВ',
	'СЕМЁНОВ',
	'ЕГОРОВ',
	'ПАВЛОВ',
	'КОЗЛОВ',
	'СТЕПАНОВ',
	'НИКОЛАЕВ',
	'ОРЛОВ',
	'АНДРЕЕВ',
	'МАКАРОВ',
	'НИКИТИН',
	'ЗАХАРОВ',
	'ЗАЙЦЕВ',
	'СОЛОВЬЁВ',
	'БОРИСОВ',
	'ЯКОВЛЕВ',
	'ГРИГОРЬЕВ',
	'РОМАНОВ',
	'ВОРОБЬЁВ',
	'СЕРГЕЕВ',
	'КОВАЛЁВ',
	'ЕЛКИН',
	'БЕЛОВ',
	'ТАРАСОВ',
	'БЕЛЯЕВ',
	'КОМАРОВ'
];
const MALE_NAMES = [
	'АЛЕКСАНДР',
	'ДМИТРИЙ',
	'МАКСИМ',
	'СЕРГЕЙ',
	'АНДРЕЙ',
	'АЛЕКСЕЙ',
	'АРТЁМ',
	'ИЛЬЯ',
	'КИРИЛЛ',
	'МИХАИЛ',
	'НИКИТА',
	'РОМАН',
	'ЕГОР',
	'ИВАН',
	'ПАВЕЛ',
	'ВИКТОР',
	'ОЛЕГ',
	'ЮРИЙ',
	'ПЁТР',
	'ВЛАДИМИР'
];
const FEMALE_NAMES = [
	'АННА',
	'МАРИЯ',
	'ЕЛЕНА',
	'ОЛЬГА',
	'НАТАЛЬЯ',
	'ТАТЬЯНА',
	'ИРИНА',
	'СВЕТЛАНА',
	'ЕКАТЕРИНА',
	'ЮЛИЯ',
	'ДАРЬЯ',
	'АЛИНА',
	'ПОЛИНА',
	'ВИКТОРИЯ',
	'КСЕНИЯ'
];
const FATHERS = [
	['АЛЕКСАНДРОВИЧ', 'АЛЕКСАНДРОВНА'],
	['ДМИТРИЕВИЧ', 'ДМИТРИЕВНА'],
	['СЕРГЕЕВИЧ', 'СЕРГЕЕВНА'],
	['АНДРЕЕВИЧ', 'АНДРЕЕВНА'],
	['ИВАНОВИЧ', 'ИВАНОВНА'],
	['ПЕТРОВИЧ', 'ПЕТРОВНА'],
	['МИХАЙЛОВИЧ', 'МИХАЙЛОВНА'],
	['ВИКТОРОВИЧ', 'ВИКТОРОВНА'],
	['ЮРЬЕВИЧ', 'ЮРЬЕВНА'],
	['ВЛАДИМИРОВИЧ', 'ВЛАДИМИРОВНА']
] as const;
const COMPANIES = ['ООО ВЕКТОР', 'ООО ГОРИЗОНТ', 'ООО РОМАШКА', 'ЗАО СЕВЕР', 'ИП ГРОМОВ'];

const MASTERS = ['АНТОН', 'СЕРГЕЙ', 'ИРИНА'];

const MALFUNCTIONS = [
	'Не включается',
	'Не заряжается',
	'Разбит экран',
	'Залит',
	'Нет звука',
	'Не ловит сеть',
	'Быстро разряжается',
	'Не работает микрофон',
	'Греется',
	'Зависает',
	'Не работает камера',
	'Не загружается',
	'Шумит вентилятор',
	'Нет изображения',
	'Не работает кнопка',
	'Сломан разъём зарядки',
	'Плохо слышно собеседника',
	'Не видит SIM-карту'
];

type Serial = 'imei' | 'sn' | 'none';
interface DeviceKind {
	type: string;
	weight: number;
	serial: Serial;
	brands: readonly (readonly [string, readonly string[]])[];
}

const DEVICES: readonly DeviceKind[] = [
	{
		type: 'Телефон',
		weight: 55,
		serial: 'imei',
		brands: [
			['Samsung', ['Galaxy A52', 'Galaxy A12', 'Galaxy S21', 'Galaxy A32']],
			['Apple', ['iPhone 11', 'iPhone 12', 'iPhone SE 2020', 'iPhone XR']],
			['Xiaomi', ['Redmi Note 10', 'Redmi 9A', 'Poco X3 Pro', 'Mi 11 Lite']],
			['Huawei', ['P30 Lite', 'Y6 2019', 'Nova 5T']],
			['Honor', ['10X Lite', '9A', '50']],
			['Realme', ['C25', '8 Pro']]
		]
	},
	{
		type: 'Планшет',
		weight: 8,
		serial: 'sn',
		brands: [
			['Apple', ['iPad 9', 'iPad Air 4']],
			['Samsung', ['Galaxy Tab A7', 'Galaxy Tab S6 Lite']],
			['Lenovo', ['Tab M10']]
		]
	},
	{
		type: 'Ноутбук',
		weight: 15,
		serial: 'sn',
		brands: [
			['Lenovo', ['IdeaPad 3', 'ThinkPad E14']],
			['Asus', ['VivoBook 15', 'TUF Gaming F15']],
			['HP', ['Pavilion 15', '250 G8']],
			['Apple', ['MacBook Air M1']]
		]
	},
	// Self-built computers have no brand or model: the table shows the type.
	{ type: 'Компьютер', weight: 5, serial: 'none', brands: [['', ['']]] },
	{
		type: 'Телевизор',
		weight: 5,
		serial: 'sn',
		brands: [
			['Samsung', ['UE43TU7100']],
			['Sony', ['KD-43X80J']],
			['Xiaomi', ['Mi TV P1 43']]
		]
	},
	{
		type: 'Игровая приставка',
		weight: 4,
		serial: 'sn',
		brands: [['Sony', ['PlayStation 4', 'PlayStation 5']]]
	},
	{
		type: 'Часы',
		weight: 4,
		serial: 'sn',
		brands: [
			['Apple', ['Watch Series 6']],
			['Samsung', ['Galaxy Watch 4']],
			['Xiaomi', ['Mi Band 6']]
		]
	},
	{
		type: 'Наушники',
		weight: 4,
		serial: 'none',
		brands: [
			['Apple', ['AirPods Pro']],
			['Sony', ['WH-1000XM4']],
			['Samsung', ['Galaxy Buds 2']]
		]
	}
];

/** Dictionaries of a demo service: the defaults of a new database plus the demo masters. */
export function makeDictionaries(): Record<
	| 'device_type'
	| 'brand'
	| 'malfunction'
	| 'completeness'
	| 'condition'
	| 'master'
	| 'warranty'
	| 'source',
	string[]
> {
	return {
		device_type: DEVICES.map((d) => d.type),
		brand: [
			'Apple',
			'Samsung',
			'Xiaomi',
			'Huawei',
			'Honor',
			'Realme',
			'Lenovo',
			'Asus',
			'HP',
			'Sony'
		],
		malfunction: [...MALFUNCTIONS],
		completeness: [
			'Устройство',
			'Чехол',
			'Зарядка',
			'Кабель',
			'Сим-карта',
			'Карта памяти',
			'Коробка'
		],
		condition: ['Без видимых повреждений', 'Царапины', 'Сколы', 'Трещина экрана', 'Следы влаги'],
		master: [...MASTERS],
		warranty: ['Без гарантии', '14 дней', '30 дней', '90 дней', '6 месяцев', '1 год'],
		source: ['Интернет', 'Рекомендация', 'Вывеска', 'Повторно', 'Объявление']
	};
}

type Rng = () => number;

/** mulberry32: small, fast and the same on every machine. */
function mulberry32(seed: number): Rng {
	let a = seed >>> 0;
	return () => {
		a = (a + 0x6d2b79f5) >>> 0;
		let t = a;
		t = Math.imul(t ^ (t >>> 15), t | 1);
		t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
		return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
	};
}

function int(rng: Rng, n: number): number {
	return Math.floor(rng() * n);
}

function pick<T>(rng: Rng, list: readonly T[]): T {
	return list[int(rng, list.length)]!;
}

function weighted<T>(rng: Rng, items: readonly (readonly [T, number])[]): T {
	let x = rng() * items.reduce((sum, [, w]) => sum + w, 0);
	for (const [value, w] of items) {
		x -= w;
		if (x < 0) return value;
	}
	return items[items.length - 1]![0];
}

function digits(rng: Rng, n: number): string {
	let s = '';
	for (let i = 0; i < n; i++) s += String(int(rng, 10));
	return s;
}

const SERIAL_CHARS = 'ABCDEFGHJKLMNPQRSTUVWXYZ0123456789';

function makeName(rng: Rng): string {
	if (rng() < 0.03) return pick(rng, COMPANIES);
	const female = rng() < 0.5;
	const surname = pick(rng, SURNAMES) + (female ? 'А' : '');
	const name = pick(rng, female ? FEMALE_NAMES : MALE_NAMES);
	if (rng() < 0.35) return `${surname} ${name} ${pick(rng, FATHERS)[female ? 1 : 0]}`;
	return `${surname} ${name}`;
}

/** 7000 and seven digits; never 70000000001, which belongs to the fixed client. */
function makePhone(rng: Rng): string {
	return '7000' + String(1_000_000 + int(rng, 9_000_000));
}

interface FakeClient {
	id: number;
	name: string;
	phone: string;
	blacklisted: boolean;
}

/** Always present: the intake demo finds this client by the phone 8 000 000-00-01. */
const FIXED_CLIENT: FakeClient = {
	id: 1,
	name: 'ЕЛКИН ПЁТР СЕРГЕЕВИЧ',
	phone: '70000000001',
	blacklisted: false
};

/**
 * «Now» of the demo data: 15 November 2026, 18:00, local time of the service. Orders are made around it,
 * so relative dates («сегодня», «вчера») and ages are counted from it, not from the real clock.
 */
export const DEMO_NOW: LocalDateTime = {
	year: 2026,
	month: 11,
	day: 15,
	hour: 18,
	minute: 0,
	second: 0
};

const MINUTE = 60_000;
const DAY = 24 * 60 * MINUTE;
const TODAY = Date.UTC(DEMO_NOW.year, DEMO_NOW.month - 1, DEMO_NOW.day);
const LAST = TODAY + (DEMO_NOW.hour * 60 + DEMO_NOW.minute) * MINUTE;
/** The newest orders are still in work. */
const OPEN_WINDOW = 60;

const p2 = (n: number) => String(n).padStart(2, '0');

/** Local date and time without offset, as the API sends it: `2026-11-15T10:24:00`. */
function localIso(ms: number): string {
	const d = new Date(ms);
	return (
		`${d.getUTCFullYear()}-${p2(d.getUTCMonth() + 1)}-${p2(d.getUTCDate())}` +
		`T${p2(d.getUTCHours())}:${p2(d.getUTCMinutes())}:00`
	);
}

const RUB = 100;

/**
 * `count` fictional orders, oldest first, numbered from 10001; the same seed gives the same orders.
 * About six orders a day over the last years, working hours 9:00–20:00; the newest ones are in work.
 */
export function makeOrders(count: number, seed = 1): OrderListItem[] {
	const rng = mulberry32(seed);
	const kindOf = new Map(DEMO_STATUSES.map((s) => [s.id, s.kind]));

	const clients: FakeClient[] = [FIXED_CLIENT];
	const poolSize = Math.max(1, Math.round(count * 0.55));
	for (let id = 2; id <= poolSize; id++) {
		clients.push({ id, name: makeName(rng), phone: makePhone(rng), blacklisted: rng() < 0.01 });
	}

	const spanDays = Math.min(8 * 365, Math.max(3, Math.ceil(count / 6)));
	const times: number[] = [];
	for (let i = 0; i < count; i++) {
		let t = TODAY - int(rng, spanDays) * DAY + (9 * 60 + int(rng, 11 * 60)) * MINUTE;
		if (t > LAST) t -= DAY;
		times.push(t);
	}
	times.sort((a, b) => a - b);

	const openWindow = Math.min(OPEN_WINDOW, Math.ceil(count / 3));
	return times.map((received, i): OrderListItem => {
		const id = 10001 + i;
		const client = i % 500 === 0 ? FIXED_CLIENT : pick(rng, clients);
		const device = weighted(
			rng,
			DEVICES.map((d) => [d, d.weight] as const)
		);
		const [brand, models] = pick(rng, device.brands);
		const model = pick(rng, models);

		const recent = count - 1 - i < openWindow;
		const statusId = recent
			? weighted(rng, [
					[1, 25],
					[2, 10],
					[3, 10],
					[4, 20],
					[5, 15],
					[6, 5],
					[7, 15]
				] as const)
			: weighted(rng, [
					[8, 87],
					[9, 1],
					[7, 8],
					[1, 1],
					[4, 2],
					[5, 1]
				] as const);
		const kind = kindOf.get(statusId) ?? 'open';
		const done = kind === 'ready' || kind === 'issued';

		const estimateFrom = rng() < 0.75 ? (5 + int(rng, 145)) * 100 * RUB : null;
		const estimateTo =
			estimateFrom !== null && rng() < 0.85 ? estimateFrom + int(rng, 30) * 100 * RUB : null;
		const finalPrice =
			done && rng() < 0.9
				? (estimateFrom ?? (5 + int(rng, 145)) * 100 * RUB) + int(rng, 20) * 50 * RUB
				: null;
		const prepayment = rng() < 0.2 ? (5 + int(rng, 26)) * 100 * RUB : 0;

		const issued =
			kind === 'issued'
				? localIso(Math.min(LAST, received + (1 + int(rng, 14)) * DAY + int(rng, 6 * 60) * MINUTE))
				: null;
		const serial =
			device.serial === 'none' || rng() < 0.25
				? ''
				: device.serial === 'imei'
					? '35' + digits(rng, 13)
					: 'S' +
						Array.from({ length: 9 }, () =>
							SERIAL_CHARS.charAt(int(rng, SERIAL_CHARS.length))
						).join('');

		return {
			id,
			received_at: localIso(received),
			issued_at: issued,
			client_id: client.id,
			client_name: client.name,
			client_phone: client.phone,
			client_blacklisted: client.blacklisted,
			service_point_id:
				rng() < 0.05
					? null
					: weighted(rng, [
							[1, 60],
							[2, 25],
							[3, 15]
						] as const),
			status_id: statusId,
			device_type: device.type,
			brand,
			model,
			serial,
			malfunction: pick(rng, MALFUNCTIONS),
			master: kind === 'diagnostics' && rng() < 0.5 ? '' : pick(rng, MASTERS),
			estimate_from: estimateFrom,
			estimate_to: estimateTo,
			prepayment,
			final_price: finalPrice,
			need_call: !done ? rng() < 0.15 : kind === 'ready' && rng() < 0.3,
			need_parts: statusId === 5 ? rng() < 0.7 : !done && rng() < 0.05,
			barcode: '2' + String(id).padStart(11, '0'),
			deleted_at: null,
			version: 1 + int(rng, 4)
		};
	});
}
