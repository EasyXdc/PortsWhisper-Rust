fn main() {
    let code = port_whisperer::run_app("whoisonport", std::env::args().skip(1).collect());
    std::process::exit(code);
}
