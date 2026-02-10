use gethostname::gethostname;
use whoami;

pub fn os() -> String {
    std::env::consts::OS.to_string()
}
pub fn hostname() -> String {
    gethostname().to_string_lossy().into_owned()
}

pub fn user() -> String {whoami::username().unwrap()}