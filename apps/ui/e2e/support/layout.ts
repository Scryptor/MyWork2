import { expect, type Locator, type Page } from '@playwright/test';

/** A monitor and the browser window on it (spec 5.3: from 1280×720 to 4K, Windows at 125 % and 150 %). */
export interface ScreenCase {
	name: string;
	screen: { width: number; height: number };
	viewport: { width: number; height: number };
	dpr: number;
	/** Manual size; none: «Авто». */
	scale?: number;
	/** `font-size` of <html> the case must end with. */
	rootPx: number;
	/** `data-width` of <html>: the band of the window's width divided by the scale. */
	band: 'xs' | 'sm' | 'md' | 'lg' | 'xl';
	/** A phone: the list is cards, and only some scenes are gone through. */
	phone?: boolean;
	/** A touch screen: `hasTouch` and `isMobile` (the pointer is coarse, the controls are 44px). */
	touch?: boolean;
}

// The window is the screen without the frame and the task bar: about 70px less in height.
export const SCREEN_CASES: readonly ScreenCase[] = [
	{
		name: '720p',
		screen: { width: 1280, height: 720 },
		viewport: { width: 1280, height: 650 },
		dpr: 1,
		rootPx: 16,
		band: 'xl'
	},
	{
		name: '768p',
		screen: { width: 1366, height: 768 },
		viewport: { width: 1366, height: 700 },
		dpr: 1,
		rootPx: 16,
		band: 'xl'
	},
	{
		name: '900p',
		screen: { width: 1440, height: 900 },
		viewport: { width: 1440, height: 830 },
		dpr: 1,
		rootPx: 16,
		band: 'xl'
	},
	{
		name: 'mbp16',
		screen: { width: 1728, height: 1117 },
		viewport: { width: 1728, height: 1050 },
		dpr: 2,
		rootPx: 16,
		band: 'xl'
	},
	{
		name: '1080p',
		screen: { width: 1920, height: 1080 },
		viewport: { width: 1920, height: 1010 },
		dpr: 1,
		rootPx: 16,
		band: 'xl'
	},
	{
		name: '1080p-half',
		screen: { width: 1920, height: 1080 },
		viewport: { width: 960, height: 1010 },
		dpr: 1,
		rootPx: 16,
		band: 'md'
	},
	{
		name: '1440p',
		screen: { width: 2560, height: 1440 },
		viewport: { width: 2560, height: 1370 },
		dpr: 1,
		rootPx: 17.6,
		band: 'xl'
	},
	{
		name: '4k',
		screen: { width: 3840, height: 2160 },
		viewport: { width: 3840, height: 2090 },
		dpr: 1,
		rootPx: 24,
		band: 'xl'
	},
	{
		name: 'win125',
		screen: { width: 1536, height: 864 },
		viewport: { width: 1536, height: 794 },
		dpr: 1.25,
		rootPx: 16,
		band: 'xl'
	},
	{
		name: 'win150',
		screen: { width: 1280, height: 720 },
		viewport: { width: 1280, height: 650 },
		dpr: 1.5,
		rootPx: 16,
		band: 'xl'
	},
	{
		name: '768p-150',
		screen: { width: 1366, height: 768 },
		viewport: { width: 1366, height: 700 },
		dpr: 1,
		scale: 1.5,
		rootPx: 24,
		band: 'md'
	},
	{
		name: '720p-90',
		screen: { width: 1280, height: 720 },
		viewport: { width: 1280, height: 650 },
		dpr: 1,
		scale: 0.9,
		rootPx: 14.4,
		band: 'xl'
	},
	// The bands between the phone and the wide window (spec 5.3, review of 5.3): 1366 at 125 % is 1093
	// wide (lg, the rail), half of 1366 is 683 (sm), a phone is 390 (xs).
	{
		name: '768p-125',
		screen: { width: 1366, height: 768 },
		viewport: { width: 1366, height: 700 },
		dpr: 1,
		scale: 1.25,
		rootPx: 20,
		band: 'lg'
	},
	{
		name: '768p-half',
		screen: { width: 1366, height: 768 },
		viewport: { width: 683, height: 700 },
		dpr: 1,
		rootPx: 16,
		band: 'sm'
	},
	{
		name: 'phone',
		screen: { width: 390, height: 844 },
		viewport: { width: 390, height: 844 },
		dpr: 3,
		rootPx: 16,
		band: 'xs',
		phone: true
	},
	// The same phone with a finger: coarse pointer, so every control takes its touch size.
	{
		name: 'phone-touch',
		screen: { width: 390, height: 844 },
		viewport: { width: 390, height: 844 },
		dpr: 3,
		rootPx: 16,
		band: 'xs',
		phone: true,
		touch: true
	}
];

type Check = 'scroll' | 'edges' | 'bars' | 'labels' | 'root';

interface CheckArg {
	check: Check;
	bars: Node[][];
	barNames: string[];
	rootPx: number;
	band: string;
}

/**
 * Runs one check inside the page and returns its offenders as text: role, name, rectangle, why.
 * Self-contained: Playwright sends the source of this function to the page.
 */
function inPage({ check, bars, barNames, rootPx, band }: CheckArg): string[] {
	// What the user clicks or types into: buttons, links, fields, tabs, menu items, and the windows.
	const INTERACTIVE = [
		'button',
		'a[href]',
		'input:not([type="hidden"])',
		'select',
		'textarea',
		'[contenteditable="true"]',
		'[contenteditable=""]',
		'[role="button"]',
		'[role="link"]',
		'[role="tab"]',
		'[role="menuitem"]',
		'[role="menuitemradio"]',
		'[role="menuitemcheckbox"]',
		'[role="textbox"]',
		'[role="searchbox"]',
		'[role="combobox"]',
		'[role="checkbox"]',
		'[role="radio"]',
		'[role="switch"]',
		'[role="dialog"]',
		'[role="alertdialog"]',
		'dialog'
	].join(',');
	const LABELLED = [
		'button',
		'[role="button"]',
		'[role="tab"]',
		'[role="menuitem"]',
		'[role="menuitemradio"]',
		'[role="menuitemcheckbox"]',
		// Ctrl K: its commands and found records are the options of a listbox.
		'[role="option"]',
		// The sidebar, the subsections of «Настройки», «открыть» and the other links.
		'a[href]',
		'label',
		'legend'
	].join(',');
	const TABLE = '[role="grid"],[role="treegrid"],[role="table"],table';
	// A strip that scrolls sideways on purpose (the quick filters on a touch screen): `data-scroll-x`.
	const STRIP = '[data-scroll-x]';
	const CELL = '[role="gridcell"],[role="columnheader"],[role="rowheader"],[role="cell"],td,th';
	const W = window.innerWidth;
	const H = window.innerHeight;

	interface Rect {
		left: number;
		top: number;
		right: number;
		bottom: number;
	}
	const r0 = (n: number) => Math.round(n);
	const box = (r: Rect) =>
		`[x=${r0(r.left)} y=${r0(r.top)} w=${r0(r.right - r.left)} h=${r0(r.bottom - r.top)}]`;
	const IMPLICIT: Record<string, string> = {
		button: 'button',
		a: 'link',
		select: 'combobox',
		textarea: 'textbox',
		dialog: 'dialog',
		label: 'label',
		legend: 'legend'
	};
	function roleOf(el: Element): string {
		const explicit = el.getAttribute('role');
		if (explicit) return explicit;
		const tag = el.tagName.toLowerCase();
		if (tag === 'input') {
			const type = (el as HTMLInputElement).type;
			if (type === 'checkbox' || type === 'radio') return type;
			return type === 'search' ? 'searchbox' : 'textbox';
		}
		return IMPLICIT[tag] ?? tag;
	}
	const squash = (s: string | null | undefined) => (s ?? '').replace(/\s+/g, ' ').trim();
	function nameOf(el: Element): string {
		const label = el.getAttribute('aria-label');
		if (label) return squash(label);
		const by = el.getAttribute('aria-labelledby');
		if (by) {
			const text = by
				.split(/\s+/)
				.map((id) => document.getElementById(id)?.textContent ?? '')
				.join(' ');
			if (squash(text)) return squash(text).slice(0, 60);
		}
		const own = (el as HTMLInputElement).labels?.[0]?.textContent;
		if (squash(own)) return squash(own).slice(0, 60);
		return (
			squash(el.textContent).slice(0, 60) ||
			squash(el.getAttribute('placeholder')) ||
			squash(el.getAttribute('title'))
		);
	}
	function tagOf(el: Element): string {
		const classes = [...el.classList].filter((c) => !c.startsWith('svelte-')).slice(0, 2);
		return el.tagName.toLowerCase() + classes.map((c) => `.${c}`).join('');
	}
	const describe = (el: Element) =>
		`${roleOf(el)} "${nameOf(el)}" ${box(el.getBoundingClientRect())}`;

	/** Transforms, filters and containment make an element the containing block of fixed descendants. */
	function transformed(s: CSSStyleDeclaration): boolean {
		return (
			s.transform !== 'none' ||
			(s.translate !== '' && s.translate !== 'none') ||
			(s.scale !== '' && s.scale !== 'none') ||
			(s.rotate !== '' && s.rotate !== 'none') ||
			s.filter !== 'none' ||
			s.perspective !== 'none' ||
			/paint|layout|strict|content/.test(s.contain)
		);
	}
	type Need = 'any' | 'positioned' | 'transformed';
	const needOf = (position: string): Need =>
		position === 'fixed' ? 'transformed' : position === 'absolute' ? 'positioned' : 'any';

	interface Clip {
		el: Element;
		x: boolean;
		y: boolean;
		/** `overflow-x: hidden | clip`: what is beside it is lost, not a scroll away. */
		cutX: boolean;
	}
	/**
	 * Ancestors whose `overflow` clips the element, nearest first: only those on its containing-block
	 * chain (an absolute or fixed element escapes the overflow of the boxes it is not positioned in).
	 * <html> and <body> clip to the window, which is checked on its own.
	 */
	function clipsOf(el: Element): Clip[] {
		const out: Clip[] = [];
		let need = needOf(getComputedStyle(el).position);
		for (let a = el.parentElement; a; a = a.parentElement) {
			const s = getComputedStyle(a);
			const isTransformed = transformed(s);
			const contains =
				need === 'any' || isTransformed || (need === 'positioned' && s.position !== 'static');
			if (!contains) continue;
			if (a !== document.documentElement && a !== document.body) {
				const x = s.overflowX !== 'visible';
				const y = s.overflowY !== 'visible';
				const cutX = s.overflowX === 'hidden' || s.overflowX === 'clip';
				if (x || y) out.push({ el: a, x, y, cutX });
			}
			need = needOf(s.position);
		}
		return out;
	}
	/** The padding box: what an overflow clip lets through (no borders, no scroll bars). */
	function clipRect(el: Element): Rect {
		const r = el.getBoundingClientRect();
		const left = r.left + el.clientLeft;
		const top = r.top + el.clientTop;
		return { left, top, right: left + el.clientWidth, bottom: top + el.clientHeight };
	}
	function scrollsSideways(el: Element): boolean {
		const s = getComputedStyle(el);
		return (
			(s.overflowX === 'auto' || s.overflowX === 'scroll') && el.scrollWidth > el.clientWidth + 1
		);
	}
	/**
	 * The table's own horizontal scroller (or one inside the table): what is in it may scroll out of sight.
	 * Not a box that merely holds a table: `main` scrolls too, and a table must not push it sideways.
	 */
	const isTableScroller = (el: Element) =>
		scrollsSideways(el) && (el.matches(TABLE) || !!el.closest(TABLE) || el.matches(STRIP));

	/**
	 * What the user sees: not hidden, not transparent, at least 2×2 (the visually hidden 1×1 helpers are
	 * not), within the height of the window and not entirely outside a clipping ancestor that scrolls
	 * (scrolled out of sight). Entirely beside an ancestor that cuts (`overflow-x: hidden | clip`), it is
	 * lost to the user, not scrolled away: it counts as seen, and check 2 names it.
	 */
	function visible(el: Element): boolean {
		if (el.closest('[aria-hidden="true"], [inert]')) return false;
		const s = getComputedStyle(el);
		if (s.visibility !== 'visible' || s.display === 'none') return false;
		if (
			typeof el.checkVisibility === 'function' &&
			!el.checkVisibility({ opacityProperty: true, visibilityProperty: true })
		)
			return false;
		const r = el.getBoundingClientRect();
		if (r.width < 2 || r.height < 2) return false;
		// Above or below the window: out of sight. Beside it: lost, and check 2 says so.
		if (r.bottom <= 0 || r.top >= H) return false;
		for (const c of clipsOf(el)) {
			const cr = clipRect(c.el);
			if (c.x && !c.cutX && (r.right <= cr.left || r.left >= cr.right)) return false;
			if (c.y && (r.bottom <= cr.top || r.top >= cr.bottom)) return false;
		}
		return true;
	}
	const all = (selector: string, root: ParentNode = document) =>
		[...root.querySelectorAll(selector)].filter(visible);

	const out: string[] = [];

	if (check === 'scroll') {
		const page = document.scrollingElement ?? document.documentElement;
		const sticking = (beyond: (r: DOMRect) => boolean) => {
			const els = [...document.body.querySelectorAll('*')].filter(
				(el) => beyond(el.getBoundingClientRect()) && getComputedStyle(el).visibility === 'visible'
			);
			return els
				.filter((el) => !els.includes(el.parentElement as Element))
				.slice(0, 5)
				.map((el) => `${tagOf(el)} ${box(el.getBoundingClientRect())}`)
				.join(', ');
		};
		if (page.scrollWidth > W)
			out.push(
				`the page scrolls sideways: scrollWidth ${page.scrollWidth} > innerWidth ${W}; ` +
					`sticking out: ${sticking((r) => r.right > W + 1)}`
			);
		if (page.scrollHeight > H)
			out.push(
				`the page scrolls down: scrollHeight ${page.scrollHeight} > innerHeight ${H}; ` +
					`sticking out: ${sticking((r) => r.bottom > H + 1)}`
			);
		// No box but a table is wider than its content box, `main` included: what sticks out is a sideways
		// scroll away (auto, scroll) or lost (hidden, clip).
		for (const el of document.body.querySelectorAll('*')) {
			if (
				el.matches(TABLE) ||
				el.closest(TABLE) ||
				(el.matches(STRIP) && scrollsSideways(el)) ||
				!visible(el)
			)
				continue;
			const s = getComputedStyle(el);
			if (!['auto', 'scroll', 'hidden', 'clip'].includes(s.overflowX)) continue;
			if (el.scrollWidth <= el.clientWidth + 1) continue;
			// Exemption: a box of text with an ellipsis is cut on purpose; check 4 judges the labels.
			if (s.textOverflow === 'ellipsis') continue;
			const inner = [...el.children]
				.filter((c) => c.getBoundingClientRect().right > clipRect(el).right + 1)
				.slice(0, 3)
				.map((c) => `${tagOf(c)} ${box(c.getBoundingClientRect())}`)
				.join(', ');
			out.push(
				`${tagOf(el)} ${box(el.getBoundingClientRect())} is wider inside ` +
					`(overflow-x ${s.overflowX}): scrollWidth ${el.scrollWidth} > clientWidth ` +
					`${el.clientWidth}; sticking out: ${inner || 'its own content'}`
			);
		}
	}

	if (check === 'edges') {
		for (const el of all(INTERACTIVE)) {
			const r = el.getBoundingClientRect();
			const clips = clipsOf(el);
			// A table scrolls sideways on purpose: its cells may be half out of sight.
			if (clips.some((c) => isTableScroller(c.el))) continue;
			if (r.left < -1 || r.right > W + 1) {
				out.push(`${describe(el)} goes beyond the window (width ${W})`);
				continue;
			}
			// A table cell clips its own content (the ellipsis of check 4); the table itself is checked.
			// The nearest clipping box, and every box farther out that cuts rather than scrolls.
			const xs = clips.filter((c) => c.x && !c.el.matches(CELL));
			const by = xs.find((c, i) => {
				if (i > 0 && !c.cutX) return false;
				const cr = clipRect(c.el);
				return r.left < cr.left - 1 || r.right > cr.right + 1;
			});
			if (by) out.push(`${describe(el)} is cut by ${tagOf(by.el)} ${box(clipRect(by.el))}`);
		}
	}

	if (check === 'bars') {
		bars.forEach((els, i) => {
			const name = barNames[i] ?? `bar ${i}`;
			if (els.length === 0) out.push(`bar not found: ${name}`);
			for (const bar of els) {
				if (!(bar instanceof Element)) continue;
				const br = bar.getBoundingClientRect();
				const found = all(INTERACTIVE, bar);
				const items = found.filter((el) => !found.some((o) => o !== el && o.contains(el)));
				const rects = items.map((el) => el.getBoundingClientRect());
				items.forEach((el, a) => {
					const r = rects[a]!;
					// What is in a strip that scrolls on purpose may be partly beyond the bar's sides.
					const strip = clipsOf(el).some((c) => isTableScroller(c.el));
					if (
						r.top < br.top - 1 ||
						r.bottom > br.bottom + 1 ||
						(!strip && (r.left < br.left - 1 || r.right > br.right + 1))
					)
						out.push(`${describe(el)} sticks out of ${name} ${box(br)}`);
					for (let b = a + 1; b < items.length; b++) {
						const q = rects[b]!;
						const dx = Math.min(r.right, q.right) - Math.max(r.left, q.left);
						const dy = Math.min(r.bottom, q.bottom) - Math.max(r.top, q.top);
						if (dx > 1 && dy > 1)
							out.push(`${describe(el)} overlaps ${describe(items[b]!)} in ${name}`);
					}
				});
			}
		});
	}

	if (check === 'labels') {
		/** A box of text that is narrower than its text: the text is cut (or spills out of it). */
		const cut = (el: Element): boolean => {
			if (!(el instanceof HTMLElement) || el.matches('input, textarea, select')) return false;
			const d = getComputedStyle(el).display;
			if (d === 'inline' || d === 'contents' || d === 'none') return false;
			if (!squash(el.textContent) || !visible(el)) return false;
			if (el.scrollWidth <= el.clientWidth + 1) return false;
			// Exemption: data cut on purpose (a long device in a list row, like a table cell; the name of a
			// point or of the user in the top bar) whose whole text is a hover away in the `title` of the
			// text or of its control. A label cut by a lack of room has no such title.
			return squash(el.closest('[title]')?.getAttribute('title')) !== squash(el.textContent);
		};
		for (const el of all(LABELLED)) {
			// Ellipsis in table cells is the norm.
			if (el.closest(CELL)) continue;
			const where = [el, ...el.querySelectorAll('*')].find(cut);
			if (where)
				out.push(
					`${describe(el)}: the label is cut (${tagOf(where)} scrollWidth ${where.scrollWidth} > ` +
						`clientWidth ${where.clientWidth})`
				);
		}
	}

	if (check === 'root') {
		const size = parseFloat(getComputedStyle(document.documentElement).fontSize);
		if (Math.abs(size - rootPx) > 0.01)
			out.push(`<html> font-size ${size}px, expected ${rootPx}px`);
		const width = document.documentElement.getAttribute('data-width');
		if (band && width !== band) out.push(`<html> data-width ${width}, expected ${band}`);
	}

	return out;
}

const CHECKS: { check: Check; title: string }[] = [
	{ check: 'scroll', title: '1. the page or a box in it scrolls sideways' },
	{ check: 'edges', title: '2. controls go beyond the window or their clipping box' },
	{ check: 'bars', title: '3. controls overlap or stick out of their bar' },
	{ check: 'labels', title: '4. labels are cut' },
	{ check: 'root', title: '5. the root font size and the width band' }
];

/**
 * Scrolls every open panel and dialog body that has more content than room to its end, and returns how many
 * it moved: what is below the fold is checked too. Waits for the scroll to settle.
 */
export async function scrollBodiesToEnd(page: Page): Promise<number> {
	const moved = await page.evaluate(() => {
		let n = 0;
		for (const layer of document.querySelectorAll(
			'[role="dialog"], [role="alertdialog"], dialog'
		)) {
			for (const el of [layer, ...layer.querySelectorAll('*')]) {
				const s = getComputedStyle(el);
				if (s.overflowY !== 'auto' && s.overflowY !== 'scroll') continue;
				if (el.scrollHeight <= el.clientHeight + 1) continue;
				el.scrollTop = el.scrollHeight;
				n++;
			}
		}
		return n;
	});
	if (moved > 0)
		await page.evaluate(
			() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done)))
		);
	return moved;
}

/** The offenders of every check, by its title: what `expectTidyLayout` asserts to be empty. */
export async function layoutOffenders(
	page: Page,
	bars: Locator[],
	{ rootPx, band }: { rootPx?: number; band?: ScreenCase['band'] } = {}
): Promise<{ title: string; offenders: string[] }[]> {
	const handles = await Promise.all(bars.map((b) => b.elementHandles()));
	try {
		const found: { title: string; offenders: string[] }[] = [];
		for (const { check, title } of CHECKS) {
			if (check === 'root' && rootPx === undefined) continue;
			const offenders = await page.evaluate(inPage, {
				check,
				bars: handles,
				barNames: bars.map((b) => b.toString()),
				rootPx: rootPx ?? 0,
				band: band ?? ''
			});
			found.push({ title, offenders });
		}
		return found;
	} finally {
		await Promise.all(handles.flat().map((h) => h.dispose()));
	}
}

/**
 * Fails with a list of offenders: overflow, overlaps, cut labels (spec 5.3, the five checks).
 * Soft: the test records the failure and goes on to its next scene, so one run names them all.
 * `bars`: the top bar, the headers and footers of panels, the intake and dialogs, the filter row; every
 * bar must be on the page. `rootPx`, `band`: the case's `font-size` and `data-width` of <html> (check 5;
 * skipped without `rootPx`). `label` is added to the titles of the failures.
 */
export async function expectTidyLayout(
	page: Page,
	bars: Locator[],
	{ rootPx, band, label = '' }: { rootPx?: number; band?: ScreenCase['band']; label?: string } = {}
): Promise<void> {
	const handles = await Promise.all(bars.map((b) => b.elementHandles()));
	try {
		for (const { check, title } of CHECKS) {
			if (check === 'root' && rootPx === undefined) continue;
			const offenders = await page.evaluate(inPage, {
				check,
				bars: handles,
				barNames: bars.map((b) => b.toString()),
				rootPx: rootPx ?? 0,
				band: band ?? ''
			});
			expect.soft(offenders, `${title}${label} (${page.url()})`).toEqual([]);
		}
	} finally {
		await Promise.all(handles.flat().map((h) => h.dispose()));
	}
}
