mod native;
#[cfg_attr(not(windows), allow(dead_code))]
mod platform;
#[cfg_attr(not(windows), allow(dead_code))]
mod runtime;
#[cfg_attr(not(windows), allow(dead_code))]
mod update;

pub fn run() {
    native::run();
}
