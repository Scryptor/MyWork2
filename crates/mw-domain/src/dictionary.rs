code_enum! {
    /// Вид справочника.
    DictionaryKind {
        DeviceType => "device_type",
        Brand => "brand",
        Color => "color",
        Completeness => "completeness",
        Condition => "condition",
        Malfunction => "malfunction",
        WorkDone => "work_done",
        Master => "master",
        ClientSource => "client_source",
    }
}

/// Значения справочников по умолчанию (справочник мастеров заполняется пользователями).
pub const DEFAULT_DICTIONARY: &[(DictionaryKind, &[&str])] = &[
    (
        DictionaryKind::DeviceType,
        &[
            "Телефон",
            "Планшет",
            "Ноутбук",
            "Компьютер",
            "Телевизор",
            "Игровая приставка",
            "Часы",
            "Наушники",
        ],
    ),
    (
        DictionaryKind::Brand,
        &[
            "Apple", "Samsung", "Xiaomi", "Huawei", "Honor", "Realme", "Lenovo", "Asus", "HP",
            "Sony",
        ],
    ),
    (
        DictionaryKind::Color,
        &[
            "Чёрный",
            "Белый",
            "Серый",
            "Серебристый",
            "Золотой",
            "Синий",
            "Красный",
            "Зелёный",
        ],
    ),
    (
        DictionaryKind::Completeness,
        &[
            "Устройство",
            "Чехол",
            "Зарядка",
            "Кабель",
            "Сим-карта",
            "Карта памяти",
            "Коробка",
        ],
    ),
    (
        DictionaryKind::Condition,
        &[
            "Без видимых повреждений",
            "Царапины",
            "Сколы",
            "Трещина экрана",
            "Следы влаги",
        ],
    ),
    (
        DictionaryKind::Malfunction,
        &[
            "Не включается",
            "Не заряжается",
            "Разбит экран",
            "Залит",
            "Нет звука",
            "Не ловит сеть",
            "Быстро разряжается",
        ],
    ),
    (
        DictionaryKind::WorkDone,
        &[
            "Диагностика",
            "Замена дисплея",
            "Замена аккумулятора",
            "Замена разъёма зарядки",
            "Чистка",
            "Прошивка",
        ],
    ),
    (DictionaryKind::Master, &[]),
    (
        DictionaryKind::ClientSource,
        &[
            "Рекомендация",
            "Интернет",
            "Авито",
            "Вывеска",
            "Повторное обращение",
        ],
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dictionary_kinds_roundtrip_and_defaults_are_unique() {
        for (kind, values) in DEFAULT_DICTIONARY {
            assert_eq!(kind.as_str().parse::<DictionaryKind>().unwrap(), *kind);
            let mut v = values.to_vec();
            v.sort();
            v.dedup();
            assert_eq!(v.len(), values.len(), "{kind:?}");
        }
    }
}
