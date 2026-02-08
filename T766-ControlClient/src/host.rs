use gethostname::gethostname;

pub fn os() -> String {
    std::env::consts::OS.to_string()
}
pub fn hostname() -> String {
    gethostname().to_string_lossy().into_owned()
}
