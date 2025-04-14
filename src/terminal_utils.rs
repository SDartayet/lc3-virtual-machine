use std::{io::stdin, os::fd::AsRawFd, process::exit};
use termios::{ECHO, ICANON, TCSANOW, Termios, tcgetattr, tcsetattr};

use crate::vm_error::VMError;

pub fn disable_input_buffering(original_tio: &mut Termios) -> Result<(), VMError> {
    tcgetattr(stdin().as_raw_fd(), original_tio).map_err(|_| VMError::TerminalIOAttributesGet)?;
    let new_tio = original_tio;
    new_tio.c_lflag &= !ICANON & !ECHO;
    tcsetattr(stdin().as_raw_fd(), TCSANOW, new_tio)
        .map_err(|_| VMError::TerminalIOAttributesSet)?;
    Ok(())
}
pub fn restore_input_buffering(original_tio: &mut Termios) -> Result<(), VMError> {
    tcsetattr(stdin().as_raw_fd(), TCSANOW, original_tio)
        .map_err(|_| VMError::TerminalIOAttributesSet)
}
pub fn handle_interrupt(original_tio: &mut Termios) -> Result<(), VMError> {
    restore_input_buffering(original_tio).map_err(|_| VMError::TerminalIOAttributesSet)?;
    exit(-2);
}
