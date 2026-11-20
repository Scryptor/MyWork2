<script lang="ts">
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import Copy from '@lucide/svelte/icons/copy';
	import Info from '@lucide/svelte/icons/info';
	import Printer from '@lucide/svelte/icons/printer';
	import Trash2 from '@lucide/svelte/icons/trash-2';
	import { goto, onNavigate } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { registerHotkey, submitShortcut } from '#lib/design/hotkeys.svelte.js';
	import { displayName } from '#lib/format/name.js';
	import {
		Button,
		ConfirmDialog,
		Dialog,
		DropdownMenu,
		Kbd,
		Popover,
		Select,
		Sheet,
		SlideOver,
		StatusBadge,
		TextInput,
		Textarea,
		toast,
		type MenuItem
	} from '#lib/ui/index.js';

	const ORDER = '№ 10481';
	const RECORDS = 5;
	const MASTERS = ['ИВАНОВ ИВАН', 'ПЕТРОВА АННА', 'СИДОРОВ ПЁТР'].map((n) => ({
		value: n,
		label: displayName(n)
	}));
	const STATUSES = {
		diagnostics: { name: 'Диагностика', color: 'amber' },
		ready: { name: 'Готов', color: 'green' }
	} as const;

	let picked = $state('');
	const MENU: MenuItem[] = [
		{ label: 'Печать', icon: Printer, shortcut: 'Mod+P', onselect: () => (picked = 'Печать') },
		{ label: 'Дублировать', icon: Copy, onselect: () => (picked = 'Дублировать') },
		{ label: 'Вернуть в работу', disabled: true, onselect: () => (picked = 'Вернуть в работу') },
		{ separator: true },
		{ label: 'Удалить', icon: Trash2, danger: true, onselect: () => (picked = 'Удалить') }
	];
	/** A heading names the items after it (a group for screen readers); arrows go over it. */
	const GROUPED: MenuItem[] = [
		{ label: 'Ждёт запчасть', onselect: () => (picked = 'Ждёт запчасть') },
		{ label: 'Согласование', onselect: () => (picked = 'Согласование') },
		{ heading: 'Выданные по статусам' },
		{ label: 'Выдан без ремонта · 12', onselect: () => (picked = 'Выдан без ремонта') },
		{ label: 'Выдан по гарантии · 3', onselect: () => (picked = 'Выдан по гарантии') }
	];

	let dialogOpen = $state(false);
	let comment = $state('');
	let commentUndone = $state(0);
	let draft = $state(true);
	let draftOpen = $state(false);
	let confirmOpen = $state(false);
	let removed = $state(0);

	let panelOpen = $state(false);
	let record = $state(1);
	let master = $state<string | null>(null);
	let note = $state('');
	let clicks = $state(0);

	let sheetOpen = $state(false);
	let intakePhone = $state('');
	let problem = $state('');
	let accepted = $state(0);

	let status = $state<keyof typeof STATUSES>('diagnostics');

	async function remove() {
		await new Promise((resolve) => setTimeout(resolve, 300));
		removed++;
	}

	function accept() {
		accepted++;
		sheetOpen = false;
		intakePhone = '';
		problem = '';
	}

	$effect(() => {
		if (sheetOpen) return submitShortcut(accept);
	});

	// N, as «Принять устройство» on the order list: a navigation, and the sheet opens while it goes.
	let sheetOnNavigate = false;
	onNavigate(() => {
		if (!sheetOnNavigate) return;
		sheetOnNavigate = false;
		sheetOpen = true;
	});
	$effect(() =>
		registerHotkey('N', () => {
			sheetOnNavigate = true;
			void goto(resolve('/design'), { replace: true });
		})
	);

	function clearComment() {
		const previous = comment;
		comment = '';
		toast.undoable('Комментарий очищен', () => {
			comment = previous;
			commentUndone++;
		});
	}

	function removeDraft() {
		draft = false;
		draftOpen = false;
	}

	function toReady() {
		if (status === 'ready') return;
		const previous = status;
		status = 'ready';
		toast.undoable(`${ORDER} → ${STATUSES.ready.name}`, () => {
			status = previous;
		});
	}
</script>

<h3>Всплывающие</h3>
<div class="row">
	<Popover label="Гарантия">
		{#snippet trigger(props)}
			<Button {...props} icon={Info}>Подробнее</Button>
		{/snippet}
		<p class="pop">Гарантия 30 дней с даты выдачи.</p>
	</Popover>
	<DropdownMenu items={MENU} label="Действия с заказом">
		{#snippet trigger(props)}
			<Button {...props} iconEnd={ChevronDown}>Действия</Button>
		{/snippet}
	</DropdownMenu>
	<DropdownMenu items={GROUPED} label="Ещё статусы">
		{#snippet trigger(props)}
			<Button {...props} variant="ghost" iconEnd={ChevronDown}>Ещё</Button>
		{/snippet}
	</DropdownMenu>
	<span class="muted">Выбрано: {picked || 'ничего'}</span>
</div>

<h3>Диалоги</h3>
<div class="row">
	<Button onclick={() => (dialogOpen = true)}>Открыть диалог</Button>
	<Button variant="danger" icon={Trash2} onclick={() => (confirmOpen = true)}>
		Удалить навсегда
	</Button>
	<span class="muted">Удалено: {removed}</span>
	{#if draft}
		<Button onclick={() => (draftOpen = true)}>Черновик заказа</Button>
	{:else}
		<Button onclick={() => (draft = true)}>Вернуть черновик</Button>
	{/if}
</div>
<Dialog
	bind:open={dialogOpen}
	title="Комментарий к заказу"
	description="Виден только сотрудникам сервиса."
>
	<div class="stack">
		<Textarea bind:value={comment} label="Комментарий" />
		<!-- «Отменить» in the toast works with the dialog open; focus comes back here. -->
		<div class="row">
			<Button onclick={clearComment}>Очистить</Button>
			<span class="muted">Отменено: {commentUndone}</span>
		</div>
	</div>
	{#snippet footer()}
		<Button onclick={() => (dialogOpen = false)}>Отмена</Button>
		<Button variant="primary" onclick={() => (dialogOpen = false)}>Сохранить</Button>
	{/snippet}
</Dialog>
<!-- The opener goes with the draft: focus moves to <main>, not to nowhere. -->
<Dialog bind:open={draftOpen} title="Черновик заказа" size="sm">
	<p class="muted">Вымышленный черновик: телефон и неисправность ещё не заполнены.</p>
	{#snippet footer()}
		<Button onclick={() => (draftOpen = false)}>Закрыть</Button>
		<Button variant="danger" icon={Trash2} onclick={removeDraft}>Удалить черновик</Button>
	{/snippet}
</Dialog>
<ConfirmDialog
	bind:open={confirmOpen}
	title="Удалить заказ {ORDER} навсегда"
	description="Заказ, его история и фото будут удалены. Вернуть их будет нельзя."
	confirmLabel="Удалить"
	onconfirm={remove}
/>

<h3>Панели</h3>
<div class="row">
	<Button onclick={() => (panelOpen = true)}>Открыть панель</Button>
	<Button onclick={() => clicks++}>Счётчик: {clicks}</Button>
	<Button onclick={() => (sheetOpen = true)}>Открыть широкую панель</Button>
	<span class="muted">Принято: {accepted}</span>
	<span class="muted">Открыть с переходом по странице: <Kbd shortcut="N" /></span>
</div>
<SlideOver
	bind:open={panelOpen}
	title="Запись № {record}"
	onprev={record > 1 ? () => record-- : undefined}
	onnext={record < RECORDS ? () => record++ : undefined}
>
	{#snippet header()}
		<p class="muted">
			Вымышленная запись {record} из {RECORDS}. Соседняя — ‹ ›, ↑↓ вне поля или
			<Kbd shortcut="Alt+ArrowDown" />
		</p>
	{/snippet}
	<div class="stack">
		<Select bind:value={master} items={MASTERS} label="Мастер" />
		<TextInput bind:value={note} label="Заметка" />
	</div>
	{#snippet footer()}
		<Button variant="primary" onclick={() => (panelOpen = false)}>Готово</Button>
	{/snippet}
</SlideOver>
<Sheet bind:open={sheetOpen} title="Новый заказ">
	<!-- Accepting is Ctrl/⌘+Enter only (submitShortcut): plain Enter in «Телефон» must not submit. -->
	<form class="stack" onsubmit={(e) => e.preventDefault()}>
		<TextInput bind:value={intakePhone} label="Телефон" type="tel" />
		<Textarea bind:value={problem} label="Неисправность" />
	</form>
	{#snippet footer()}
		<Button onclick={() => (sheetOpen = false)}>Отмена</Button>
		<Button variant="primary" shortcut="Mod+Enter" onclick={accept}>Принять</Button>
	{/snippet}
</Sheet>

<h3>Уведомления</h3>
<p>
	Статус заказа {ORDER}:
	<StatusBadge name={STATUSES[status].name} color={STATUSES[status].color} />
</p>
<div class="row">
	<Button onclick={toReady}>Статус → Готов</Button>
	<Button onclick={() => toast.show({ message: 'Отправлено на принтер «Приёмка»' })}>
		Уведомление
	</Button>
	<Button onclick={() => toast.show({ message: 'Сохранено', tone: 'success' })}>Успех</Button>
	<Button
		onclick={() =>
			toast.show({
				message: 'Не удалось сохранить: нет связи с сервером',
				tone: 'danger',
				action: {
					label: 'Повторить',
					run: () => {
						toast.show({ message: 'Сохранено', tone: 'success' });
					}
				}
			})}
	>
		Ошибка
	</Button>
	<span class="muted">Отменить последнее вне поля: <Kbd shortcut="Mod+Z" /></span>
</div>

<style>
	.row {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
		margin-bottom: var(--space-3);
	}
	.stack {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}
	.muted {
		margin: 0;
		color: var(--text-3);
	}
	.pop {
		margin: 0;
		max-width: 16rem;
	}
</style>
