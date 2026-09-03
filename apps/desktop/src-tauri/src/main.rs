// Evita que se abra una consola extra en Windows en release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    myglossmusic_lib::run()
}
