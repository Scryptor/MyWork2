//! Разбор грязных значений старой базы. Каждый разбор сообщает «пусто» или
//! «не разобрано», ничего не угадывая молча.

pub(crate) mod dates;
pub(crate) mod flags;
pub(crate) mod lists;
pub(crate) mod money;
pub(crate) mod phones;
pub(crate) mod states;
pub(crate) mod tracking;

pub(crate) use dates::{DateValue, parse_date};
pub(crate) use flags::parse_flag;
pub(crate) use money::{MoneyValue, SUSPICIOUS_RUBLES, parse_money};
pub(crate) use phones::split_phones;
pub(crate) use states::{StateEvent, read_states_row};
pub(crate) use tracking::extract_tracking;
