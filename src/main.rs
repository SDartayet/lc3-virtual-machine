use lc3vm::*;
use std::{env, io::stdin, os::fd::AsRawFd, path::Path, process::exit};
use termios::{ECHO, ICANON, TCSANOW, Termios, tcgetattr, tcsetattr};

mod lc3vm;

fn disable_input_buffering(original_tio: &mut Termios) -> Result<(), VMError> {
    tcgetattr(stdin().as_raw_fd(), original_tio).map_err(|_| VMError::TerminalIOAttributesGet)?;
    let new_tio = original_tio;
    new_tio.c_lflag &= !ICANON & !ECHO;
    tcsetattr(stdin().as_raw_fd(), TCSANOW, new_tio)
        .map_err(|_| VMError::TerminalIOAttributesSet)?;
    Ok(())
}

fn restore_input_buffering(original_tio: &mut Termios) -> Result<(), VMError> {
    tcsetattr(stdin().as_raw_fd(), TCSANOW, original_tio)
        .map_err(|_| VMError::TerminalIOAttributesSet)
}

fn handle_interrupt(original_tio: &mut Termios) -> Result<(), VMError> {
    restore_input_buffering(original_tio).map_err(|_| VMError::TerminalIOAttributesSet)?;
    exit(-2);
}

fn main() {
    let mut vm = LC3VM::new();
    if let Ok(mut original_tio) = Termios::from_fd(stdin().as_raw_fd()) {
        ctrlc::set_handler(move || {
            let _ = handle_interrupt(&mut original_tio);
        })
        .expect("Error setting Ctrl-C handler");

        if let Err(error) = disable_input_buffering(&mut original_tio) {
            vm.handle_error(error);
        }

        let args: Vec<String> = env::args().collect();

        if args.len() < 2 {
            /* show usage string */
            println!("Usage: make run path=[path to binary]");
            exit(2);
        }

        for item in args.iter().skip(1) {
            let file_path = Path::new(&item);

            if !vm.read_image_file(file_path) {
                println!("failed to load image: {}", item);
                exit(1);
            }
        }

        while vm.running {
            if let Ok(instruction_code) = vm.decode_instruction() {
                vm.execute_instruction(instruction_code);
            }
        }
    } else {
        vm.handle_error(VMError::TerminalIOAttributesGet);
    }
}
