// The intake form without the view (spec 6.5): the draft kept on this device, what is required, the new
// order it becomes, and the checks of the right column.
import type { AppSettings } from '$api/AppSettings';
import type { ClientPhone } from '$api/ClientPhone';
import type { NewOrder } from '$api/NewOrder';
import type { OrderListItem } from '$api/OrderListItem';
import type { Status } from '$api/Status';
import type { WarrantyTerm } from '$api/WarrantyTerm';
import { parseLocalDateTime, type LocalDateTime } from '#lib/format/date.js';
import { phoneDigits } from '#lib/format/phone.js';
import { normalizeSearch } from '#lib/format/search.js';
import type { IntakePrefill } from './intake.svelte.js';

/** The form of a new order as typed so far; money in kopecks. */
export interface IntakeDraft {
	/** The client chosen among the found ones; null — a new client from the fields below. */
	clientId: number | null;
	/** «Новый клиент» was pressed although clients with this phone were found. */
	newClient: boolean;
	fullName: string;
	/** At least one: the first is the phone field the form starts with. Digits only. */
	phones: ClientPhone[];
	source: string;
	address: string;
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
	master: string;
	statusId: number | null;
	estimateFrom: number | null;
	estimateTo: number | null;
	prepayment: number | null;
	warranty: string;
	comment: string;
	/**
	 * The order already accepted from this draft: its answer came while the sheet, opened again, was
	 * being changed, so the draft stayed. The sheet says so until «Отмена» or the next «Принять».
	 */
	acceptedAs: number | null;
}

/** Kept in localStorage of this device until the order is accepted or the intake cancelled. */
export const INTAKE_DRAFT_KEY = 'mw.intake.draft';

/** Phones and serials are looked up from this many characters. */
export const LOOKUP_MIN = 5;

const byOrder = (a: Status, b: Status) => a.sort_order - b.sort_order || a.id - b.id;

/** A new order starts in work: the open and diagnostics statuses that are not archived, in their order. */
export function intakeStatuses(statuses: readonly Status[]): Status[] {
	return statuses
		.filter((s) => !s.archived && (s.kind === 'open' || s.kind === 'diagnostics'))
		.sort(byOrder);
}

const blankPhone = (): ClientPhone => ({ phone: '', note: '' });

/** Master and warranty from the settings, the status is the first of kind «Диагностика». */
export function emptyDraft(settings: AppSettings, statuses: readonly Status[]): IntakeDraft {
	return {
		clientId: null,
		newClient: false,
		fullName: '',
		phones: [blankPhone()],
		source: '',
		address: '',
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
		master: settings.default_master,
		statusId: intakeStatuses(statuses).find((s) => s.kind === 'diagnostics')?.id ?? null,
		estimateFrom: null,
		estimateTo: null,
		prepayment: null,
		warranty: settings.default_warranty,
		comment: '',
		acceptedAs: null
	};
}

// Readers of a stored record: a value of a wrong type is not there (shared with the demo of /design).

type Stored = Record<string, unknown>;

export function readText(o: Stored, key: string, fallback = ''): string {
	const v = o[key];
	return typeof v === 'string' ? v : fallback;
}

export function readList(o: Stored, key: string): string[] {
	const v = o[key];
	return Array.isArray(v) ? v.filter((x): x is string => typeof x === 'string') : [];
}

/** Chips, also as one text (R16). */
function readChips(o: Stored, key: string): string[] {
	const v = o[key];
	return typeof v === 'string' ? splitChips(v) : readList(o, key);
}

export function readMoney(o: Stored, key: string): number | null {
	const v = o[key];
	return Number.isSafeInteger(v) && (v as number) >= 0 ? (v as number) : null;
}

/** The record stored under a key, or null for nothing or garbage. */
export function readRecord(raw: string | null): Stored | null {
	let obj: unknown;
	try {
		obj = raw === null ? null : JSON.parse(raw);
	} catch {
		return null;
	}
	return typeof obj === 'object' && obj !== null && !Array.isArray(obj) ? (obj as Stored) : null;
}

function readPhones(o: Stored): ClientPhone[] {
	const v = o.phones;
	const phones = (Array.isArray(v) ? v : []).flatMap((p: unknown) => {
		if (typeof p !== 'object' || p === null) return [];
		const { phone, note } = p as Stored;
		return typeof phone === 'string'
			? [{ phone: phoneDigits(phone), note: typeof note === 'string' ? note : '' }]
			: [];
	});
	return phones.length > 0 ? phones : [blankPhone()];
}

/** Never throws: anything missing or of a wrong type is the empty draft's value. */
export function parseDraft(
	raw: string | null,
	settings: AppSettings,
	statuses: readonly Status[]
): IntakeDraft {
	const base = emptyDraft(settings, statuses);
	const o = readRecord(raw);
	if (o === null) return base;
	const id = (v: unknown) => (Number.isSafeInteger(v) && (v as number) > 0 ? (v as number) : null);
	const statusId = id(o.statusId);
	return {
		clientId: id(o.clientId),
		newClient: o.newClient === true,
		fullName: readText(o, 'fullName'),
		phones: readPhones(o),
		source: readText(o, 'source'),
		address: readText(o, 'address'),
		deviceType: readText(o, 'deviceType'),
		brand: readText(o, 'brand'),
		model: readText(o, 'model'),
		color: readText(o, 'color'),
		serial: readText(o, 'serial'),
		password: readText(o, 'password'),
		completeness: readChips(o, 'completeness'),
		condition: readChips(o, 'condition'),
		malfunctionChips: readChips(o, 'malfunctionChips'),
		malfunction: readText(o, 'malfunction'),
		master: readText(o, 'master', base.master),
		statusId: intakeStatuses(statuses).some((s) => s.id === statusId) ? statusId : base.statusId,
		estimateFrom: readMoney(o, 'estimateFrom'),
		estimateTo: readMoney(o, 'estimateTo'),
		prepayment: readMoney(o, 'prepayment'),
		warranty: readText(o, 'warranty', base.warranty),
		comment: readText(o, 'comment'),
		acceptedAs: id(o.acceptedAs)
	};
}

/**
 * Two drafts of one order: the same client and the same device. Other changes (the comment, the money)
 * do not make another order of it.
 */
export function sameOrder(a: IntakeDraft, b: IntakeDraft): boolean {
	const key = (d: IntakeDraft) =>
		JSON.stringify([
			d.clientId,
			d.fullName.trim(),
			d.phones.map((p) => p.phone),
			...[d.deviceType, d.brand, d.model, d.serial].map((v) => v.trim())
		]);
	return key(a) === key(b);
}

/** Intake for a client (from the client's card): their draft stays, a draft of anyone else is replaced. */
export function withPrefill(
	d: IntakeDraft,
	prefill: IntakePrefill,
	settings: AppSettings,
	statuses: readonly Status[]
): IntakeDraft {
	const id = prefill.clientId;
	if (id === undefined || d.clientId === id) return d;
	return { ...emptyDraft(settings, statuses), clientId: id };
}

// Chips of «Комплектность» and «Внешнее состояние» are one text in the order (R16).

export function joinChips(chips: readonly string[]): string {
	return chips
		.map((c) => c.trim())
		.filter(Boolean)
		.join(', ');
}

export function splitChips(text: string): string[] {
	return text
		.split(',')
		.map((c) => c.trim())
		.filter(Boolean);
}

export type RequiredKey = 'contact' | 'deviceType' | 'brand' | 'model' | 'malfunction';
export const REQUIRED_LABELS: Record<RequiredKey, string> = {
	contact: 'Телефон или ФИО',
	deviceType: 'Тип устройства',
	brand: 'Бренд',
	model: 'Модель',
	malfunction: 'Неисправность'
};

/** A phone counts from five digits: a small town number «2-12-34» is one too. */
const MIN_PHONE_DIGITS = 5;

/**
 * Spec 6.5: a phone or a name (or a found client), the device type, the malfunction (text or a chip).
 * `strict` is the setting «intake_required_fields»: then the server also wants the brand and the model.
 * In the order of the form.
 */
export function missingRequired(d: IntakeDraft, strict = false): RequiredKey[] {
	const missing: RequiredKey[] = [];
	const blank = (v: string) => v.trim() === '';
	const phone = d.phones.some((p) => phoneDigits(p.phone).length >= MIN_PHONE_DIGITS);
	if (d.clientId === null && !phone && blank(d.fullName)) missing.push('contact');
	if (blank(d.deviceType)) missing.push('deviceType');
	if (strict && blank(d.brand)) missing.push('brand');
	if (strict && blank(d.model)) missing.push('model');
	if (blank(d.malfunction) && d.malfunctionChips.length === 0) missing.push('malfunction');
	return missing;
}

/** The order the draft becomes; text trimmed (except the password), money in kopecks. */
export function toNewOrder(d: IntakeDraft, pointId: number | null): NewOrder {
	const phones = d.phones
		.map((p) => ({ phone: phoneDigits(p.phone), note: p.note.trim() }))
		.filter((p) => p.phone !== '' || p.note !== '');
	return {
		client:
			d.clientId !== null
				? { existing: d.clientId }
				: {
						new: {
							full_name: d.fullName.trim(),
							phones,
							address: d.address.trim(),
							note: '',
							source: d.source.trim(),
							blacklisted: false
						}
					},
		service_point_id: pointId,
		status_id: d.statusId,
		device_type: d.deviceType.trim(),
		brand: d.brand.trim(),
		model: d.model.trim(),
		serial: d.serial.trim(),
		color: d.color.trim(),
		device_password: d.password,
		completeness: joinChips(d.completeness),
		device_condition: joinChips(d.condition),
		malfunction: joinChips([...d.malfunctionChips, d.malfunction]),
		internal_comment: d.comment.trim(),
		master: d.master.trim(),
		estimate_from: d.estimateFrom,
		estimate_to: d.estimateTo,
		prepayment: d.prepayment ?? 0,
		warranty: d.warranty.trim(),
		need_call: false,
		need_parts: false,
		attachment_draft: null,
		request_key: null
	};
}

/** Suggestions of a field: the dictionary in its order, then values of past orders not in it. */
export function mergeSuggestions(
	dictionary: readonly string[],
	values: readonly string[]
): string[] {
	const seen = new Set<string>();
	const out: string[] = [];
	for (const v of [...dictionary, ...values]) {
		const text = v.trim();
		const key = normalizeSearch(text);
		if (key === '' || seen.has(key)) continue;
		seen.add(key);
		out.push(text);
	}
	return out;
}

// The right column.

/** The digits of the first phone to look clients up by, from five digits. */
export function phoneQuery(d: IntakeDraft): string | null {
	const digits = phoneDigits(d.phones[0]?.phone ?? '');
	return digits.length >= LOOKUP_MIN ? digits : null;
}

/** The serial to look past orders up by, from five characters. */
export function serialQuery(serial: string): string | null {
	const text = serial.trim();
	return text.length >= LOOKUP_MIN ? text : null;
}

/** «Этот IMEI уже был»: of the orders the search found, those whose serial has the typed text. */
export function serialHits(orders: readonly OrderListItem[], serial: string): OrderListItem[] {
	const key = normalizeSearch(serial);
	return key === '' ? [] : orders.filter((o) => normalizeSearch(o.serial).includes(key));
}

const asMillis = (t: LocalDateTime) =>
	Date.UTC(t.year, t.month - 1, t.day, t.hour, t.minute, t.second);
const daysIn = (year: number, month: number) => new Date(Date.UTC(year, month, 0)).getUTCDate();

/** The end of the warranty after the issue; null without one («без гарантии» is 0 of any unit). */
export function warrantyUntil(issuedAt: string, term: WarrantyTerm | null): LocalDateTime | null {
	const t = parseLocalDateTime(issuedAt);
	if (t === null || term === null || term.value <= 0) return null;
	if (term.unit === 'days') {
		const d = new Date(asMillis(t) + term.value * 86_400_000);
		return { ...t, year: d.getUTCFullYear(), month: d.getUTCMonth() + 1, day: d.getUTCDate() };
	}
	const months = t.month - 1 + term.value;
	const year = t.year + Math.floor(months / 12);
	const month = (months % 12) + 1;
	return { ...t, year, month, day: Math.min(t.day, daysIn(year, month)) };
}

/** The end of a warranty still running at `now`, or null. */
export function underWarranty(
	issuedAt: string,
	term: WarrantyTerm | null,
	now: LocalDateTime
): LocalDateTime | null {
	const until = warrantyUntil(issuedAt, term);
	return until !== null && asMillis(until) > asMillis(now) ? until : null;
}
