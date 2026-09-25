#[no_mangle]
pub extern "C" fn amemory_probe() -> u32 {
    0xA013
}

#[no_mangle]
pub extern "C" fn amemory_cpu_step(value: u32) -> u32 {
    value.wrapping_add(1)
}
