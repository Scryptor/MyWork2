<script lang="ts">
	import Plus from '@lucide/svelte/icons/plus';
	import Search from '@lucide/svelte/icons/search';
	import Inbox from '@lucide/svelte/icons/inbox';
	import {
		Avatar,
		Badge,
		Button,
		Callout,
		Card,
		Divider,
		EmptyState,
		FlagIcons,
		IconButton,
		Kbd,
		KeyValue,
		Section,
		Skeleton,
		Spinner,
		StatusBadge,
		Timeline
	} from '#lib/ui/index.js';

	const VARIANTS = ['primary', 'secondary', 'ghost', 'danger'] as const;
	const SIZES = ['sm', 'md', 'lg'] as const;
	const TONES = ['neutral', 'accent', 'danger', 'warning', 'success', 'info'] as const;
	const STATUSES = [
		['Диагностика', 'amber'],
		['Согласование с клиентом', 'violet'],
		['Согласовано', 'indigo'],
		['Принят в работу', 'blue'],
		['Ждёт запчасть', 'red'],
		['Принят по гарантии', 'teal'],
		['Готов', 'green'],
		['Выдан', 'gray']
	] as const;
	const NOW = { year: 2026, month: 11, day: 15, hour: 12, minute: 0, second: 0 };
	const EVENTS = [
		{ id: 1, at: '2026-11-15T10:30:00', who: 'ИВАНОВ ИВАН', text: 'Принят в работу' },
		{ id: 2, at: '2026-11-14T16:05:00', who: 'ПЕТРОВА АННА', text: 'Согласована цена' },
		{ id: 3, at: '2026-10-02T09:00:00', text: 'Принят в ремонт' }
	];
</script>

<h3>Кнопки</h3>
{#each VARIANTS as variant (variant)}
	<div class="row">
		{#each SIZES as size (size)}
			<Button {variant} {size}>{variant} {size}</Button>
		{/each}
		<Button {variant} icon={Plus} shortcut="N">С иконкой</Button>
		<Button {variant} busy>Занято</Button>
		<Button {variant} disabled>Недоступно</Button>
	</div>
{/each}
<div class="row">
	<IconButton label="Найти" icon={Search} />
	<IconButton label="Добавить" icon={Plus} variant="secondary" />
	<Kbd shortcut="Mod+K" />
	<Spinner />
	<Skeleton width="8rem" />
	<Divider vertical />
</div>

<h3>Метки и статусы</h3>
<div class="row">
	{#each TONES as tone (tone)}
		<Badge {tone}>{tone}</Badge>
	{/each}
</div>
<div class="row">
	{#each STATUSES as [name, color] (name)}
		<StatusBadge {name} {color} />
	{/each}
</div>
<div class="row">
	<Avatar name="ИВАНОВ ИВАН" />
	<Avatar name="ПЕТРОВА АННА" size="sm" />
	<FlagIcons needCall needParts blacklisted />
</div>

<h3>Блоки</h3>
<Card>
	<Section title="Раздел">
		<KeyValue
			items={[
				{ label: 'Клиент', value: 'ИВАНОВ ИВАН' },
				{ label: 'Телефон', value: '+7 000 000-00-01' },
				{ label: 'Email', value: 'не указан', muted: true }
			]}
		/>
	</Section>
</Card>
<Card padding="none">
	<EmptyState icon={Inbox} title="Заказов нет" description="Создайте первый заказ">
		{#snippet actions()}<Button variant="primary" icon={Plus}>Новый заказ</Button>{/snippet}
	</EmptyState>
</Card>
<Callout tone="info" title="Информация">Обычное сообщение.</Callout>
<Callout tone="warning">Предупреждение.</Callout>
<Callout tone="danger" title="Ошибка">Не удалось сохранить.</Callout>
<Callout tone="success">Готово.</Callout>
<!-- region: a named block of a page, as on the order card. -->
<Section title="История" region>
	<Timeline items={EVENTS} now={NOW} emptyText="Событий нет" />
</Section>

<style>
	.row {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
		margin-bottom: var(--space-3);
	}
</style>
