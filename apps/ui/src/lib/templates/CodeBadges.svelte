<!-- The row of badges under the code of a «Typst-код» block: a click puts the badge where the caret was
     (the caret after it, or inside its `[ ]` by `caret_back`; the focus in the editor); the last one,
     «Другое поле…», searches all fields. -->
<script lang="ts">
	import type { DocumentField } from '$api/DocumentField';
	import type { DocumentKind } from '$api/DocumentKind';
	import { app } from '#lib/app/context.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import InsertChip from '#lib/ui/InsertChip.svelte';
	import { badgeText, linkBadges, staticBadges } from './code-badges.js';
	import InsertField from './InsertField.svelte';
	import { inTypstCode, insertIntoField } from './insert.js';

	let {
		kind,
		target,
		value,
		onchange
	}: {
		/** The document the template is of: «Другое поле…» offers its fields. */
		kind: DocumentKind;
		/** The code editor the badges put text into. */
		target: HTMLTextAreaElement | null;
		value: string;
		onchange: (value: string) => void;
	} = $props();

	const t = ru.templates;
	const badges = $derived([...staticBadges(), ...linkBadges(app.settings.firm_links)]);

	const asCode = (f: DocumentField, before: string) =>
		`${inTypstCode(before) ? '' : '#'}v("${f.key}")`;
</script>

<div class="badges" role="group" aria-label={t.badgesLabel}>
	{#each badges as badge (badge.id)}
		<InsertChip
			label={badge.label}
			code={badge.markup}
			onclick={() =>
				insertIntoField(target, value, (b) => badgeText(badge, b), onchange, badge.caret_back)}
		/>
	{/each}
	<InsertField {kind} {target} {value} {onchange} format={asCode} trigger="chip" />
</div>

<style>
	.badges {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-start;
		gap: var(--space-2);
	}
</style>
