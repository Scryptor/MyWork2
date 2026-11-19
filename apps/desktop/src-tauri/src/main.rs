// Без консольного окна на Windows в сборке для людей.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    mw_desktop::run();
}
