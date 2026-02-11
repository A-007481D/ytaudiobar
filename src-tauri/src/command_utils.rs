use tokio::process::Command as TokioCommand;
use std::process::Command as StdCommand;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

/// Creates a new async Command with CREATE_NO_WINDOW flag automatically applied on Windows.
/// This prevents console windows from flashing when spawning processes.
pub fn command_no_window(program: &str) -> TokioCommand {
    let mut cmd = TokioCommand::new(program);

    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    cmd
}

/// Creates a new blocking Command with CREATE_NO_WINDOW flag automatically applied on Windows.
/// This prevents console windows from flashing when spawning processes.
pub fn command_no_window_blocking(program: &str) -> StdCommand {
    let mut cmd = StdCommand::new(program);

    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    cmd
}
