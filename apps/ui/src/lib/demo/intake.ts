// Logic of the intake demo (spec 6.5): the draft kept on the device, what is required, the client and
// the IMEI found among the fictional orders. What the real intake shares lives in #lib/intake/draft.js.
import type { OrderListItem } from '$api/OrderListItem';
import {
	readList,
	readMoney,
	readRecord,
	readText,
	REQUIRED_LABELS,
	type RequiredKey as IntakeRequiredKey
} from '#lib/intake/draft.js';

export { REQUIRED_LABELS };
/** The demo has no setting «intake_required_fields»: the three fields of spec 6.5. */
export type RequiredKey = Exclude<IntakeRequiredKey, 'brand' | 'model'>;

/** The form of a new order as typed so far; money in kopecks. */
export interface IntakeDraft {
	/** Digits only. */
	phone: string;
	name: string;
	source: string;
	deviceType: string;
	brand: string;
	model: string;
	color: string;
	serial: string;
	password: string;
	completeness: string[];
	condition: string[];
	malfunctionChips: string[];
	malfunction: string;
	master: string | null;
	statusId: number | null;
	estimateFrom: number | null;
	estimateTo: number | null;
	prepayment: number | null;
	warranty: string;
	comment: string;
}

/** Kept in localStorage of this device, like the device settings. */
export const INTAKE_DRAFT_KEY = 'mw.demo.intakeDraft';
export const DEFAULT_WARRANTY = '30 дней';

export function emptyDraft(): IntakeDraft {
	return {
		phone: '',
		name: '',
		source: '',
		deviceType: '',
		brand: '',
		model: '',
		color: '',
		serial: '',
		password: '',
		completeness: [],
		condition: [],
		malfunctionChips: [],
		malfunction: '',
		master: null,
		statusId: null,
		estimateFrom: null,
		estimateTo: null,
		prepayment: null,
		warranty: DEFAULT_WARRANTY,
		comment: ''
	};
}

/** Never throws: anything missing or of a wrong type falls back to the empty draft's value. */
export function parseDraft(raw: string | null): IntakeDraft {
	const base = emptyDraft();
	const o = readRecord(raw);
	if (o === null) return base;
	const text = (k: keyof IntakeDraft, fallback: string) => readText(o, k, fallback);
	const list = (k: keyof IntakeDraft) => readList(o, k);
	const money = (k: keyof IntakeDraft) => readMoney(o, k);
	return {
		phone: text('phone', '').replace(/\D/g, ''),
		name: text('name', ''),
		source: text('source', ''),
		deviceType: text('deviceType', ''),
		brand: text('brand', ''),
		model: text('model', ''),
		color: text('color', ''),
		serial: text('serial', ''),
		password: text('password', ''),
		completeness: list('completeness'),
		condition: list('condition'),
		malfunctionChips: list('malfunctionChips'),
		malfunction: text('malfunction', ''),
		master: typeof o.master === 'string' ? o.master : null,
		statusId: Number.isSafeInteger(o.statusId) ? (o.statusId as number) : null,
		estimateFrom: money('estimateFrom'),
		estimateTo: money('estimateTo'),
		prepayment: money('prepayment'),
		warranty: text('warranty', base.warranty),
		comment: text('comment', '')
	};
}

/** A phone counts from six digits: a city number «12-34-56» is one too. */
const MIN_PHONE_DIGITS = 6;

/** Spec 6.5: a phone or a name, the device type, the malfunction (text or a chip). */
export function missingRequired(d: IntakeDraft): RequiredKey[] {
	const missing: RequiredKey[] = [];
	if (d.phone.replace(/\D/g, '').length < MIN_PHONE_DIGITS && d.name.trim() === '') {
		missing.push('contact');
	}
	if (d.deviceType.trim() === '') missing.push('deviceType');
	if (d.malfunction.trim() === '' && d.malfunctionChips.length === 0) missing.push('malfunction');
	return missing;
}

/** Phones are compared by their last ten digits: «8 000 …» and «+7 000 …» are one number. */
export function phoneKey(digits: string): string | null {
	const d = digits.replace(/\D/g, '');
	return d.length >= 10 ? d.slice(-10) : null;
}

export interface FoundClient {
	id: number;
	name: string;
	phone: string;
	blacklisted: boolean;
	/** Newest first. */
	orders: OrderListItem[];
}

const newestFirst = (a: OrderListItem, b: OrderListItem) =>
	a.received_at < b.received_at ? 1 : a.received_at > b.received_at ? -1 : b.id - a.id;

/** The client with this phone among the orders, with all their orders. */
export function findClient(digits: string, orders: readonly OrderListItem[]): FoundClient | null {
	const key = phoneKey(digits);
	if (key === null) return null;
	const first = orders.find((o) => phoneKey(o.client_phone) === key);
	if (!first) return null;
	return {
		id: first.client_id,
		name: first.client_name,
		phone: first.client_phone,
		blacklisted: first.client_blacklisted,
		orders: orders.filter((o) => o.client_id === first.client_id).sort(newestFirst)
	};
}

/** «Этот IMEI уже был»: the newest order with this IMEI or serial (from six characters). */
export function findSerial(serial: string, orders: readonly OrderListItem[]): OrderListItem | null {
	const key = serial.trim().toUpperCase();
	if (key.length < 6) return null;
	let found: OrderListItem | null = null;
	for (const o of orders) {
		if (o.serial.toUpperCase() === key && (found === null || newestFirst(o, found) < 0)) found = o;
	}
	return found;
}
