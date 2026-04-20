use gethostname::gethostname;

pub fn os() -> &'static str {
    std::env::consts::OS
}

pub fn hostname() -> String {
    gethostname().to_string_lossy().into_owned()
}

