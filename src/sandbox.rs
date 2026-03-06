use anyhow::Result;
use mentci_box_lib::{Sandbox, SandboxConfig};

pub fn run_from_args(mut args: Vec<String>) -> Result<()> {
    if args.first().map(String::as_str) == Some("sandbox") {
        args.remove(0);
    }
    // SandboxConfig doesn't have from_args anymore, using a default for now
    // Since mentci-box is the primary tool now.
    let config = SandboxConfig {
        workdir: std::env::current_dir()?,
        home: std::env::temp_dir().join("mentci-aid-sandbox"),
        share_network: false,
        binds: vec![],
        ro_binds: vec![],
        command: if args.is_empty() { vec!["/bin/sh".to_string()] } else { args },
        env_map: std::collections::HashMap::new(),
    };
    let sandbox = Sandbox::from_config(config);
    sandbox.run()
}
