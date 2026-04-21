fn main() {
    let mut counter: u64 = 0;
    loop {
        counter = counter.wrapping_add(1);
        if counter % 1_000_000 == 0 {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
}
