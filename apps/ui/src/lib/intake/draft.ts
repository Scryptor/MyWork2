// The intake form without the view (spec 6.5): the draft kept on this device, what is required, the new
// order it becomes, and the checks of the right column.
import type { AppSettings } from '$api/AppSettings';
import type { Client } from '$api/Client';
import type { ClientRef } from '$api/ClientRef';
import type { ClientPhone } from '$api/ClientPhone';
import type { NewOrder } from '$api/NewOrder';
import type { OrderListItem } from '$api/OrderListItem';
import type { Status } from '$api/Status';
import type { WarrantyTerm } from '$api/WarrantyTerm';
import { parseLocalDateTime, type LocalDateTime } from '#lib/format/date.js';
import { removeStorage } from '#lib/design/storage.js';
import { phoneDigits, phoneKey } from '#lib/format/phone.js';
import { normalizeSearch } from '#lib/format/search.js';
import { isPattern } from '#lib/orders/pattern.js';
import type { IntakePrefill } from './intake.svelte.js';

/**
 * The order already accepted from a draft: its answer came while the sheet, opened again, was being
 * changed, so the draft stayed. It belongs to the key it was sent with.
 */
export interface AcceptedMark {
	/** The number of the order. */
	id: number;
	/** The key of the draft (and of the attempt) it was accepted with. */
	key: string;
	/** The client of that order: choosing another one makes a new draft of the form. */
	clientId: number;
}

/** The form of a new order as typed so far; money in kopecks. */
export interface IntakeDraft {
	/**
	 * The user who began the draft; null in a draft of before this field (it is claimed by the first user
	 * who opens it) and without sign-in. A draft of another user is not offered (`claimDraft`).
	 */
	owner: number | null;
	/**
	 * Made with the draft and kept with it: the photos wait on the server under it, and it is the key of
	 * the attempt (`request_key`), so «Принять» sent again gets the same order. A new draft, a new key.
	 */
	key: string;
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
	/** The unlock pattern: the points in their order, «1596»; empty — none. */
	pattern: string;
	completeness: string[];
	condition: string[];
	malfunctionChips: string[];
	malfunction: string;
	/** The master — a user marked «Мастер»; null — not chosen. */
	masterId: number | null;
	statusId: number | null;
	estimateFrom: number | null;
	estimateTo: number | null;
	prepayment: number | null;
	warranty: string;
	comment: string;
	/**
	 * What an earlier «Принять» of this key asked for (`attemptOf`) when it may have made the order: no
	 * answer came in time, or the answer came late. Until «Отмена» or another client, photos stay as they
	 * are (they would not go to that order), and an order that comes back for the key closes the sheet
	 * only if the draft is still what was asked.
	 */
	attempt: string | null;
	/**
	 * The order already accepted from this draft. Edits keep it; the sheet says so until «Отмена»,
	 * «Принять» or another client is chosen.
	 */
	acceptedAs: AcceptedMark | null;
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

/** What the server takes as a draft and attempt key. */
const KEY_PATTERN = /^[A-Za-z0-9-]{1,64}$/;

/**
 * A UUID. `crypto.randomUUID` exists only on a secure address (https or this computer): a page opened by
 * the address of the network makes it from `getRandomValues`, which is there always.
 */
export function newDraftKey(): string {
	if (typeof crypto.randomUUID === 'function') return crypto.randomUUID();
	const b = crypto.getRandomValues(new Uint8Array(16));
	b[6] = (b[6]! & 0x0f) | 0x40;
	b[8] = (b[8]! & 0x3f) | 0x80;
	const hex = Array.from(b, (x) => x.toString(16).padStart(2, '0')).join('');
	return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

/** Master and warranty from the settings, the status is the first of kind «Диагностика». */
export function emptyDraft(settings: AppSettings, statuses: readonly Status[]): IntakeDraft {
	return {
		owner: null,
		key: newDraftKey(),
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
		pattern: '',
		completeness: [],
		condition: [],
		malfunctionChips: [],
		malfunction: '',
		masterId: settings.default_master_user_id,
		statusId: intakeStatuses(statuses).find((s) => s.kind === 'diagnostics')?.id ?? null,
		estimateFrom: null,
		estimateTo: null,
		prepayment: null,
		warranty: settings.default_warranty,
		comment: '',
		attempt: null,
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

const positiveId = (v: unknown) =>
	Number.isSafeInteger(v) && (v as number) > 0 ? (v as number) : null;

/** A mark of the draft's own key, or none: a mark of 5.1 (a bare number) has no key to belong to. */
function readMark(v: unknown, key: string): AcceptedMark | null {
	if (typeof v !== 'object' || v === null) return null;
	const o = v as Stored;
	const id = positiveId(o.id);
	const clientId = positiveId(o.clientId);
	return id !== null && clientId !== null && o.key === key ? { id, key, clientId } : null;
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

/**
 * The stored draft as `userId` may see it: another user's draft is dropped (null), one without an owner
 * (stored before owners were kept) is claimed once, one of their own stays as it is. Without a user
 * (no sign-in) nothing is checked.
 */
export function claimDraft(raw: string | null, userId: number | null): string | null {
	if (raw === null || userId === null) return raw;
	const o = readRecord(raw);
	if (o === null) return raw;
	const owner = positiveId(o.owner);
	if (owner === null) return JSON.stringify({ ...o, owner: userId });
	return owner === userId ? raw : null;
}

/** The draft of this device is removed: the user signed out, nobody else may see a client's data. */
export function forgetDraft(): void {
	removeStorage(INTAKE_DRAFT_KEY);
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
	const statusId = positiveId(o.statusId);
	// A draft of 5.1 has no key: it gets the new one.
	const key = typeof o.key === 'string' && KEY_PATTERN.test(o.key) ? o.key : base.key;
	return {
		owner: positiveId(o.owner),
		key,
		clientId: positiveId(o.clientId),
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
		// A pattern the server would not take (three points, a repeat) is none: it is entered again.
		pattern: isPattern(readText(o, 'pattern')) ? readText(o, 'pattern') : '',
		completeness: readChips(o, 'completeness'),
		condition: readChips(o, 'condition'),
		malfunctionChips: readChips(o, 'malfunctionChips'),
		malfunction: readText(o, 'malfunction'),
		// A draft of an older version kept the master as text: it gets the default master.
		masterId: 'masterId' in o ? positiveId(o.masterId) : base.masterId,
		statusId: intakeStatuses(statuses).some((s) => s.id === statusId) ? statusId : base.statusId,
		estimateFrom: readMoney(o, 'estimateFrom'),
		estimateTo: readMoney(o, 'estimateTo'),
		prepayment: readMoney(o, 'prepayment'),
		warranty: readText(o, 'warranty', base.warranty),
		comment: readText(o, 'comment'),
		attempt: typeof o.attempt === 'string' ? o.attempt : null,
		acceptedAs: readMark(o.acceptedAs, key)
	};
}

/**
 * The answer to an order sent before the sheet was closed (or closed and opened again), about the draft
 * there is now. `close`: it is the very draft that was sent, it goes. `mark`: the same draft (its key)
 * edited since then, whatever was edited — it stays, marked as accepted. `other`: a draft begun after it.
 */
export function lateAnswer(now: IntakeDraft, sent: IntakeDraft): 'close' | 'mark' | 'other' {
	// A repeat of «Принять» after no answer, of a draft changed since the first attempt: the order is
	// that of the first one, without the changes — marked, not closed.
	if (JSON.stringify(now) === JSON.stringify(sent))
		return acceptedAnswer(now) === 'keep' ? 'mark' : 'close';
	return now.key === sent.key ? 'mark' : 'other';
}

/** What a «Принять» of the draft asks the server for, as text: the order without its point and photos. */
export function attemptOf(d: IntakeDraft): string {
	return JSON.stringify(toNewOrder(d, null));
}

/**
 * Why photos cannot be added or removed now: the order is accepted (`accepted`), or an earlier «Принять»
 * got no answer and may have made it (`unanswered`); null — they can.
 */
export function photoLock(d: IntakeDraft): 'accepted' | 'unanswered' | null {
	if (d.acceptedAs !== null) return 'accepted';
	return d.attempt !== null ? 'unanswered' : null;
}

/**
 * The photos of the draft may be the order's: it is accepted, or an attempt got no answer and its
 * `POST /orders` may still attach them. Their key is not released (deleted) when the draft goes — the
 * server's cleanup after 24 h takes what is left.
 */
export function photosMayBeOrders(d: IntakeDraft): boolean {
	return d.acceptedAs !== null || d.attempt !== null;
}

/**
 * «Принять» got its order back. `close`: the order is the draft as it is. `keep`: the order was made by
 * an earlier attempt of this key and the draft was changed since, so the changes are not in it — the
 * sheet stays and says so.
 */
export function acceptedAnswer(d: IntakeDraft): 'close' | 'keep' {
	return d.attempt !== null && d.attempt !== attemptOf(d) ? 'keep' : 'close';
}

/** The client an attempt was sent for: an existing one or the fields of a new one. */
function attemptClient(attempt: string): ClientRef | null {
	try {
		return (JSON.parse(attempt) as NewOrder).client;
	} catch {
		return null;
	}
}

/**
 * The client chosen is not the one an earlier attempt of the draft was for: this is another order, with
 * a key of its own. Nothing sent yet — the same draft. A new client sent may have been made by the
 * server: a client with one of its phones (found by it after Esc and N) is that client.
 */
export function startsAnotherOrder(d: IntakeDraft, client: Pick<Client, 'id' | 'phones'>): boolean {
	if (d.acceptedAs !== null) return d.acceptedAs.clientId !== client.id;
	if (d.attempt === null) return false;
	const sent = attemptClient(d.attempt);
	if (sent === null) return true;
	if ('existing' in sent) return sent.existing !== client.id;
	const keys = sent.new.phones.map((p) => phoneKey(p.phone)).filter((k) => k !== '');
	return !client.phones.some((p) => keys.includes(phoneKey(p.phone)));
}

/**
 * Intake for a client (from the client's card): their draft stays, a draft of anyone else is replaced,
 * and so is a draft already accepted.
 */
export function withPrefill(
	d: IntakeDraft,
	prefill: IntakePrefill,
	settings: AppSettings,
	statuses: readonly Status[]
): IntakeDraft {
	const id = prefill.clientId;
	if (id === undefined || (d.clientId === id && d.acceptedAs === null)) return d;
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

/**
 * The order the draft becomes; text trimmed (except the password), money in kopecks. The key of the
 * draft is the key of the attempt, and the photos of the draft go with it when it has any.
 */
export function toNewOrder(d: IntakeDraft, pointId: number | null, photos = false): NewOrder {
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
		device_pattern: d.pattern,
		completeness: joinChips(d.completeness),
		device_condition: joinChips(d.condition),
		malfunction: joinChips([...d.malfunctionChips, d.malfunction]),
		internal_comment: d.comment.trim(),
		master_user_id: d.masterId,
		estimate_from: d.estimateFrom,
		estimate_to: d.estimateTo,
		prepayment: d.prepayment ?? 0,
		warranty: d.warranty.trim(),
		need_call: false,
		need_parts: false,
		attachment_draft: photos ? d.key : null,
		request_key: d.key
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
