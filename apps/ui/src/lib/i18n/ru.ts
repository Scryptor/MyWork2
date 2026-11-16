export const ru = {
	common: {
		close: 'Закрыть',
		cancel: 'Отмена',
		undo: 'Отменить',
		retry: 'Повторить',
		more: 'Ещё',
		loading: 'Загрузка…',
		nothingFound: 'Ничего не найдено',
		back: 'Назад'
	},
	api: { offline: 'Нет связи с сервером', failed: 'Не удалось выполнить запрос' },
	flags: { needCall: 'Позвонить', needParts: 'Заказать запчасть', blacklisted: 'Чёрный список' },
	fields: {
		moneyError: 'Введите сумму, например 6 500',
		custom: '+ своё',
		select: 'Выберите',
		openCalendar: 'Открыть календарь',
		datePlaceholder: 'дд.мм.гггг',
		dateError: 'Введите дату, например 15.11.2026',
		dateRangeError: 'Дата вне допустимого диапазона',
		prevMonth: 'Предыдущий месяц',
		nextMonth: 'Следующий месяц',
		showPassword: 'Показать пароль',
		hidePassword: 'Скрыть пароль'
	},
	save: { saving: 'Сохраняю…', saved: 'Сохранено', failed: 'Не сохранено' },
	panel: { prev: 'Предыдущая запись', next: 'Следующая запись' },
	toast: { region: 'Уведомления' },
	shell: {
		nav: 'Разделы',
		bottomNav: 'Основная навигация',
		attention: 'Требует внимания'
	},
	palette: {
		title: 'Найти или выполнить',
		placeholder: 'Найти заказ, клиента или команду…',
		empty: 'Ничего не найдено',
		recent: 'Недавние',
		orders: 'Заказы',
		clients: 'Клиенты',
		commands: 'Команды'
	},
	app: {
		name: 'MyWork',
		orders: 'Заказы',
		intake: 'Принять',
		intakeDevice: 'Принять устройство',
		point: 'Точка обслуживания',
		allPoints: 'Все точки',
		pointButton: (name: string | null) => `Точка: ${name ?? 'все'}`,
		lightTheme: 'Светлая тема',
		darkTheme: 'Тёмная тема',
		theme: 'Тема',
		themes: { system: 'Как в системе', light: 'Светлая', dark: 'Тёмная' },
		themeCommand: { system: 'Тема: как в системе', light: 'Тема: светлая', dark: 'Тема: тёмная' },
		userMenu: (name: string) => `Пользователь: ${name}`,
		changePassword: 'Сменить пароль',
		logout: 'Выйти',
		attention: 'Требуют внимания',
		attentionItems: {
			call: 'Позвонить',
			parts: 'Заказать запчасть',
			longDiagnostics: 'Долгая диагностика',
			longReady: 'Давно готовы'
		},
		allClear: 'Всё в порядке ✓',
		views: 'Мои виды',
		undoFailed: (message: string) => `Не удалось отменить: ${message}`
	},
	scan: {
		barcodeNotFound: (code: string) => `Заказ со штрихкодом ${code} не найден`,
		numberNotFound: (id: number) => `Заказ № ${id} не найден`,
		chooseTitle: 'Несколько заказов с этим штрихкодом',
		chooseDescription: (code: string) => `Штрихкод ${code}. Выберите заказ`
	},
	auth: {
		title: 'Вход в MyWork',
		login: 'Логин',
		password: 'Пароль',
		remember: 'Запомнить на этом устройстве',
		submit: 'Войти',
		loginRequired: 'Введите логин',
		passwordRequired: 'Введите пароль',
		invalidCredentials: 'Неверный логин или пароль',
		tooManyAttempts: (secs: number) => `Слишком много попыток. Повторите через ${secs} с`
	},
	password: {
		title: 'Смена пароля',
		current: 'Текущий пароль',
		next: 'Новый пароль',
		repeat: 'Повторите новый пароль',
		submit: 'Сменить пароль',
		required: 'Заполните поле',
		mismatch: 'Пароли не совпадают',
		wrongCurrent: 'Неверный текущий пароль',
		tooShort: 'Не короче 4 символов',
		changed: 'Пароль изменён. Войдите с новым паролем'
	},
	shortcuts: {
		title: 'Клавиши',
		general: 'Общие',
		find: 'Найти или выполнить',
		intake: 'Принять устройство',
		undo: 'Отменить последнее действие',
		help: 'Справка по клавишам',
		close: 'Закрыть',
		panel: 'Панель заказа',
		neighbour: 'Соседний заказ'
	},
	media: {
		photos: 'Фото устройства',
		addPhoto: 'Добавить фото',
		removePhoto: 'Удалить фото',
		of: 'из',
		files: 'Файлы',
		chooseFiles: 'Выбрать файлы',
		dropHint: 'Перетащите файлы сюда или вставьте из буфера'
	},
	status: { label: 'Статус' },
	table: {
		columns: 'Колонки',
		moveUp: 'Выше',
		moveDown: 'Ниже',
		selectAll: 'Выбрать все видимые',
		selectRow: 'Выбрать строку',
		selected: 'Выбрано',
		selection: 'Выбранные строки',
		clearSelection: 'Снять выделение',
		empty: 'Нет записей'
	}
} as const;
