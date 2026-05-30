use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cli {
    /// YAML workspace template
    pub template: PathBuf,

    /// Session name used for persisted focus and viewport state
    pub session: String,
}

impl Cli {
    pub fn parse() -> Self {
        let mut args = std::env::args().skip(1);
        let mut template = PathBuf::from("./examples/web-reader.yaml");
        let mut session = String::from("main");

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--session" => {
                    if let Some(value) = args.next() {
                        session = value;
                    }
                }
                value if value.starts_with("--session=") => {
                    session = value[10..].to_owned();
                }
                value if value.starts_with('-') => {
                    // Ignore unknown flags for the MVP.
                }
                value => {
                    template = PathBuf::from(value);
                }
            }
        }

        Self { template, session }
    }
}
