//! Что должно быть заполнено в заказе перед выдачей (настройка «Обязательно при выдаче»).

code_enum! {
    /// Поле заказа, обязательное при выдаче.
    IssueRequirement {
        FinalPrice => "final_price",
        WorkDone => "work_done",
        Master => "master",
        Warranty => "warranty",
    }
}
