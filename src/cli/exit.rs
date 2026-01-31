use eyre::Result;

pub fn exit_with_code(code: i32, message: &str) -> Result<()> {
    eprintln!("{message}");
    std::process::exit(code);
}
