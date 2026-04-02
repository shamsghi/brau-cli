use std::env;
use std::io::{self, IsTerminal};
use std::thread;

#[derive(Clone, Copy)]
pub struct MotionSettings {
    pub animations_enabled: bool,
    pub finale_enabled: bool,
}

pub fn should_run_motion(motion: MotionSettings) -> bool {
    if !motion.animations_enabled {
        return false;
    }

    let is_terminal = io::stdout().is_terminal();
    let no_color = env::var_os("NO_COLOR").is_some();
    let clicolor_disabled = matches!(env::var("CLICOLOR"), Ok(value) if value == "0");
    let dumb_term = matches!(env::var("TERM"), Ok(value) if value == "dumb");

    is_terminal
        && !no_color
        && !clicolor_disabled
        && !dumb_term
        && env::var_os("BRAU_NO_ANIM").is_none()
        && env::var_os("CI").is_none()
}

pub fn run_with_motion<T, W, A>(enabled: bool, work: W, animate: A) -> Result<T, String>
where
    T: Send,
    W: FnOnce() -> T + Send,
    A: FnOnce(),
{
    if !enabled {
        return Ok(work());
    }

    thread::scope(|scope| {
        let work_handle = scope.spawn(work);
        animate();
        work_handle
            .join()
            .map_err(|_| "A background task stopped unexpectedly.".to_string())
    })
}
