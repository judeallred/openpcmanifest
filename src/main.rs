//! `OpenPCManifest` entry point: the GUI with no arguments, the CLI otherwise.

// Release GUI builds use the Windows GUI subsystem so double-clicking the exe
// does not open a console window. CLI mode re-attaches to the parent console.
#![cfg_attr(
    all(windows, feature = "gui", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use std::process::ExitCode;

use openpcmanifest::cli::{self, Command};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match cli::parse(&args) {
        Ok(Command::Gui) => gui(),
        Ok(cmd) => {
            console::attach();
            ExitCode::from(cli::run(&cmd))
        }
        Err(message) => {
            console::attach();
            let _ = std::io::Write::write_all(
                &mut std::io::stderr(),
                format!("openpcmanifest: {message}\n").as_bytes(),
            );
            ExitCode::from(cli::EXIT_USAGE)
        }
    }
}

#[cfg(feature = "gui")]
fn gui() -> ExitCode {
    match openpcmanifest::gui::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            console::attach();
            let _ = std::io::Write::write_all(
                &mut std::io::stderr(),
                format!("openpcmanifest: could not open the window: {e}\n").as_bytes(),
            );
            ExitCode::FAILURE
        }
    }
}

#[cfg(not(feature = "gui"))]
fn gui() -> ExitCode {
    ExitCode::from(cli::run(&Command::Help))
}

#[cfg(windows)]
#[allow(unsafe_code, reason = "one Win32 call to attach to the parent console")]
mod console {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn AttachConsole(process_id: u32) -> i32;
    }

    /// `ATTACH_PARENT_PROCESS`: attach to the console of the launching shell.
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;

    /// Attaches to the parent console so CLI output appears when the exe is
    /// built for the GUI subsystem. Harmless if already attached.
    pub fn attach() {
        // SAFETY: AttachConsole takes a plain integer, touches no memory we
        // own, and only fails (returning 0) if there is no parent console.
        let _ = unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
    }
}

#[cfg(not(windows))]
mod console {
    /// Consoles need no special handling outside Windows.
    pub const fn attach() {}
}
