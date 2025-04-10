use ::termios::tcgetattr;
use Flag::*;
use GeneralPurposeRegister::*;
use OpCode::*;
use TrapCode::*;
use std::{
    env,
    fs::File,
    io::{Read, Write, stdin, stdout},
    ops::{BitAnd, BitOr, BitOrAssign, Index, IndexMut},
    os::fd::AsRawFd,
    path::Path,
    process::exit,
};
use termios::{ECHO, ICANON, TCSANOW, Termios, tcsetattr};

//The number of memory addresses is 2^16
const MAX_MEMORY_ADDRESS: usize = 1 << 16;
const MEMORY_REGISTER_KEYBOARD_STATUS_ADDRESS: usize = 0xFE00;
const MEMORY_REGISTER_KEYBOARD_DATA_ADDRESS: usize = 0xFE02;

const THREE_BIT_MASK: u16 = 0b111;
const FIVE_BIT_MASK: u16 = 0b11111;
const SIX_BIT_MASK: u16 = 0b111111;
const EIGHT_BIT_MASK: u16 = 0xFF;
const NINE_BIT_MASK: u16 = 0x01FF;
const TEN_BIT_MASK: u16 = 0x7FF;

///The registers the architecture contains.
/// R0 to R7 are the general purpose registers. PC is the instruction pointer.
/// RCOND is the flags register,
enum GeneralPurposeRegister {
    R0 = 0,
    R1 = 1,
    R2 = 2,
    R3 = 3,
    R4 = 4,
    R5 = 5,
    R6 = 6,
    R7 = 7,
}
enum TrapCode {
    TrapGETC = 0x20,  /* get character from keyboard, not echoed onto the terminal */
    TrapOUT = 0x21,   /* output a character */
    TrapPUTS = 0x22,  /* output a word string */
    TrapIN = 0x23,    /* get character from keyboard, echoed onto the terminal */
    TrapPUTSP = 0x24, /* output a byte string */
    TrapHALT = 0x25,  /* halt the program */
}
#[derive(Copy, Clone, Debug)]
enum VMError {
    InvalidOpCode,
    InvalidTrapCode,
    TerminalIOAttributesGet,
    TerminalIOAttributesSet,
}

/// The opcodes for the instructions the architecture supports
#[derive(Debug)]
enum OpCode {
    OpBR = 0b0000 << 12,   /* branch */
    OpADD = 0b0001 << 12,  /* add  */
    OpLD = 0b0010 << 12,   /* load */
    OpST = 0b0011 << 12,   /* store */
    OpJSR = 0b0100 << 12,  /* jump register */
    OpAND = 0b0101 << 12,  /* bitwise and */
    OpLDR = 0b0110 << 12,  /* load register */
    OpSTR = 0b0111 << 12,  /* store register */
    OpNOT = 0b1001 << 12,  /* bitwise not */
    OpLDI = 0b1010 << 12,  /* load indirect */
    OpSTI = 0b1011 << 12,  /* store indirect */
    OpJMP = 0b1100 << 12,  /* jump */
    OpLEA = 0b1110 << 12,  /* load effective address */
    OpTRAP = 0b1111 << 12, /* execute trap */
}

enum Flag {
    Positive = 0b001, /* Set when the result of the previous operation was positive */
    Zero = 0b010,     /* Set when the result of the previous operation was zero */
    Negative = 0b100, /* Set when the result of the previous operation was negative */
}

fn extend_sign_for_integer(value: u16, value_bit_count: u16) -> u16 {
    // If the first bit of the value is negative, because of how two's complement works, we need to extend it with ones unti lwe have 16 bits to preserve the sign
    // I check if the first bit of the value is one, and if it is I extend it with ones, otherwise with zeroes
    let is_immediate_negative = (value & (1 << (value_bit_count - 1))) != 0;
    // If the sign extension is negative, I fill with 11 bits of 1, since 11+5 = 16. Otherwise I don't need to do anything because the filler would be zeroes
    if is_immediate_negative {
        // Doing a bitwise or between the sign extension I need and the immediate value I get the immediate with the sign extended
        // The sign extension will be a series of ones left of an amount of zeroes equivalent to the bits of my current value
        value | (0xFFFF << value_bit_count)
    } else {
        value
    }
}

fn disable_input_buffering(original_tio: &mut Termios) -> Result<(), VMError> {
    if let Err(_) = tcgetattr(stdin().as_raw_fd(), original_tio) {
        return Err(VMError::TerminalIOAttributesGet);
    }
    let new_tio = original_tio;
    new_tio.c_lflag &= !ICANON & !ECHO;
    if let Err(_) = tcsetattr(stdin().as_raw_fd(), TCSANOW, new_tio) {
        return Err(VMError::TerminalIOAttributesSet);
    }
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

pub struct LC3VM {
    general_registers: [u16; 8],
    program_counter: u16,
    condition_flags: u16,
    memory: [u16; MAX_MEMORY_ADDRESS],
    running: bool,
    current_instruction: u16,
}

impl LC3VM {
    fn new() -> LC3VM {
        LC3VM {
            general_registers: [0; 8],
            program_counter: 0x3000,
            condition_flags: 0,
            memory: [0; MAX_MEMORY_ADDRESS],
            running: true,
            current_instruction: 0,
        }
    }

    /// Updates the condition flags according to the result of the last arithmetic operations
    /// Input: the register on which the result was stored
    fn update_flags(&mut self, register_number: usize) {
        //I reset the condition flags first
        self.condition_flags = 0;
        let register_value = self.general_registers[register_number];
        if register_value == 0 {
            self.condition_flags |= Zero;
        } else if register_value & (1 << 15) != 0 {
            //In two's complement, if the first bit is one, the number is negative
            self.condition_flags |= Negative;
        } else {
            self.condition_flags |= Positive;
        }
    }

    fn handle_error(&mut self, error_type: VMError) {
        self.running = false;
        let mut print_vm_state = false;

        match error_type {
            VMError::InvalidOpCode => {
                println!("Invalid OpCode was provided");
                print_vm_state = true;
            }
            VMError::InvalidTrapCode => {
                println!("Invalid TrapCode was provided");
                print_vm_state = true;
            }
            VMError::TerminalIOAttributesGet => {
                println!("Error getting terminal io parameters");
            }
            VMError::TerminalIOAttributesSet => {
                println!("Error setting terminal io parameters");
            }
        }
        if print_vm_state {
            println!("VM State at time of failure:");
            println!("Program counter: {}", self.program_counter);
            println!(
                "Instruction code: {:016b}",
                self.memory[self.program_counter as usize]
            );
            println!("Flag values: {}", self.condition_flags & THREE_BIT_MASK);
            for register_num in 0..=7 {
                println!(
                    "Register {}: {}",
                    register_num, self.general_registers[register_num]
                );
            }
        }
    }

    fn read_memory_and_check_keyboard_input(&mut self, index: usize) -> u16 {
        if index == MEMORY_REGISTER_KEYBOARD_STATUS_ADDRESS {
            let mut input_buffer = [1; 1];
            let Ok(_) = stdin().read_exact(&mut input_buffer) else {
                panic!("Error reading from standard input");
            };
            if input_buffer[0] != 0 {
                self.memory[MEMORY_REGISTER_KEYBOARD_STATUS_ADDRESS] = 1 << 15;
                self.memory[MEMORY_REGISTER_KEYBOARD_DATA_ADDRESS] = input_buffer[0] as u16;
            } else {
                self.memory[MEMORY_REGISTER_KEYBOARD_STATUS_ADDRESS] = 0;
            }
        }
        self.memory[index]
    }

    fn decode_instruction(&mut self) -> Result<OpCode, VMError> {
        self.current_instruction =
            self.read_memory_and_check_keyboard_input(self.program_counter as usize);
        let parse_result = OpCode::try_from(self.memory[self.program_counter as usize]);

        match parse_result {
            Ok(opcode) => Ok(opcode),
            Err(error) => {
                self.handle_error(error);
                Err(error)
            }
        }
    }

    fn execute_instruction(&mut self, instruction_code: OpCode) {
        self.program_counter = self.program_counter.wrapping_add(1);
        //println!("{:?}", instruction_code);
        match instruction_code {
            OpADD => self.add(),
            OpAND => self.and(),
            OpNOT => self.not(),
            OpBR => self.branch(),
            OpJMP => self.jump(),
            OpJSR => self.jump_register_or_offset(),
            OpLD => self.load(),
            OpLEA => self.load_address(),
            OpLDR => self.load_register(),
            OpST => self.store(),
            OpSTR => self.store_register(),
            OpSTI => self.store_indirect(),
            OpTRAP => self.execute_trap_routine(),
            OpLDI => self.load_indirect(),
        }
    }

    /// Implements the addition instruciton
    /// Instruction structure: OPCODE (4 bits) | Destination register (3 bits) | Source register 1 (3 bits) | Mode (immediate or another register, 1 bit) | [00 | Source register 2 (3 bits)] on mode 0 or a 5 bit value on mode 1
    fn add(&mut self) {
        let instruction = self.current_instruction;
        //I "push" the bits for the register number to the rightmost position, and make all the other bits 0 by doing a bitwise AND with 0b111
        let source_register_1_number = ((instruction >> 6) & THREE_BIT_MASK) as usize;

        //Same as previously, but I need to shift right by 9 bits given there are 9 bits befrore the destination register
        let destination_register_number = ((instruction >> 9) & THREE_BIT_MASK) as usize;

        //Mode flag is just one bit, so the bitwise AND is with 0b1. If it's zero I use register mode, if one immediate mode
        let mode_flag = (instruction >> 5) & 0b1 == 0;

        let second_operand = if mode_flag {
            // I get the number of the second register from the last 3 bits
            let source_register_2_number = (instruction & THREE_BIT_MASK) as usize;
            self.general_registers[source_register_2_number]
        } else {
            let five_bit_immediate = instruction & FIVE_BIT_MASK; // I filter the first 5 bits of the instruction, which contain the immediate, and set the rest to zero
            extend_sign_for_integer(five_bit_immediate, 5)
        };

        //Wrapping add lets us recreate the way addition works in two's complement systems while keeping the values unsigned (for more generalization)
        self.general_registers[destination_register_number] =
            self.general_registers[source_register_1_number].wrapping_add(second_operand);

        self.update_flags(destination_register_number);
    }

    /// Implements the bitwise NOT instruction
    /// Instruction structure: OPCODE (4 bits) | Destination register (3 bits) | Source register (3 bits) | 111111
    fn not(&mut self) {
        let instruction = self.current_instruction;
        //I "push" the bits for the register number to the rightmost position, and make all the other bits 0 by doing a bitwise AND with 0b111
        let source_register_number = ((instruction >> 6) & THREE_BIT_MASK) as usize;

        //Same as previously, but I need to shift right by 9 bits given there are 9 bits befrore the destination register
        let destination_register_number = ((instruction >> 9) & THREE_BIT_MASK) as usize;

        self.general_registers[destination_register_number] =
            !self.general_registers[source_register_number];

        self.update_flags(destination_register_number);
    }

    /// Implements the bitwise AND instruction
    /// Instruction structure: OPCODE (4 bits) | Destination register (3 bits) | Source register 1 (3 bits) | Mode (immediate or another register, 1 bit) | [00 Source register 2 (3 bits)] on mode 0 or a 5 bit value on mode 1
    fn and(&mut self) {
        let instruction = self.current_instruction;
        //I "push" the bits for the register number to the rightmost position, and make all the other bits 0 by doing a bitwise AND with 0b111
        let source_register_1_number = ((instruction >> 6) & THREE_BIT_MASK) as usize;

        //Same as previously, but I need to shift right by 9 bits given there are 9 bits befrore the destination register
        let destination_register_number = ((instruction >> 9) & THREE_BIT_MASK) as usize;

        //Mode flag is just one bit, so the bitwise AND is with 0b1. If it's zero I use register mode, if one immediate mode
        let mode_flag = (instruction >> 5) & 0b1 == 0;

        let second_operand = if mode_flag {
            let source_register_2_number = (instruction & THREE_BIT_MASK) as usize;
            self.general_registers[source_register_2_number]
        } else {
            let five_bit_immediate = instruction & FIVE_BIT_MASK; // I filter the first 5 bits of the instruction, which contain the immediate, and set the rest to zero
            extend_sign_for_integer(five_bit_immediate, 5)
        };

        self.general_registers[destination_register_number] =
            self.general_registers[source_register_1_number] & second_operand;

        self.update_flags(destination_register_number);
    }

    /// Jumps a set number of instructions if zero, negative or positive flags are up (depending on encoding)
    /// Instruction structure: OPCODE (4 bits) | Negative flag (1 bit) | Zero flag (1 bit) | Positive flag (1 bit) | Offset from current PC value to which to jump (9 bits)
    fn branch(&mut self) {
        let instruction = self.current_instruction;
        //The 9 rightmost bits contain the offset I need to jump when the condition is true
        //ox1FF is 9 bits set to 1
        let program_counter_offset = extend_sign_for_integer(instruction & NINE_BIT_MASK, 9);

        //Starting left, bit 10 is that the condition is the positive flag being up, bit 11 is the zero one being up, and bit 12 the negative one
        //I shift 9 rightward and and the value with 0b111 to get the value of the three
        let flag_values = (instruction >> 9) & THREE_BIT_MASK;
        let condition_flag_is_up = if flag_values != 0 {
            self.condition_flags & flag_values != 0
        } else {
            false
        };

        if condition_flag_is_up {
            self.program_counter = self.program_counter.wrapping_add(program_counter_offset);
        }
    }

    /// Makes PC jump to the memory address in the register indicated by the instruction
    /// Instruction structure: OPCODE (4 bits) | 000 | Number of the register with the memory address (3 bits) | 000000
    fn jump(&mut self) {
        let instruction = self.current_instruction;
        //I get the destination register by skipping the filler zeroes and getting the three bits that come after that
        let destination_register = ((instruction >> 6) & THREE_BIT_MASK) as usize;
        self.program_counter = self.general_registers[destination_register];
    }

    /// Makes PC jump to the memory address in the register indicated by the instruction or to an offset, depending on the mode
    /// Instruction structure: OPCODE (4 bits) | Mode (0 for register and 1 for offset, 1 bit) | [00 | Number of the register with the memory address (3 bits) | 000000] in register mode or 11 bit offset in offset mode
    fn jump_register_or_offset(&mut self) {
        let instruction = self.current_instruction;
        // I get the 11th bit. If it's zero then I need to use register mode, if it's one I need to use offset mode
        let is_register_mode = ((instruction >> 11) & 1) == 0;

        self.general_registers[R7] = self.program_counter;

        if is_register_mode {
            //I get the destination register by skipping the filler zeroes and getting the three bits that come after that
            let destination_register = ((instruction >> 6) & THREE_BIT_MASK) as usize;
            self.program_counter = self.general_registers[destination_register];
        } else {
            let offset = extend_sign_for_integer(instruction & TEN_BIT_MASK, 11); // 0x7FF consists of 11 bits of ones; I want to get the rightmost 11 bits which contain the offset
            self.program_counter = self.program_counter.wrapping_add(offset);
        }
    }

    /// Loads a value from memory into a register. The address is an offset from the program counter
    /// Structure: Opcode (4 bits) | Destination register number (3 bits) | Offset from program counter to be loaded from memory (9 bits)
    fn load(&mut self) {
        let instruction = self.current_instruction;
        //I "push" the bits for the register number to the rightmost position, and make all the other bits 0 by doing a bitwise AND with 0b111
        let destination_register = (instruction >> 9) & THREE_BIT_MASK;
        let offset = extend_sign_for_integer(instruction & NINE_BIT_MASK, 9);
        self.general_registers[destination_register as usize] = self
            .read_memory_and_check_keyboard_input(
                (self.program_counter.wrapping_add(offset)) as usize,
            );
        self.update_flags(destination_register as usize);
    }

    /// Loads a value from memory into a register. The address is an offset from a register dictated by the instruction
    /// Structure: Opcode (4 bits) | Destination register number (3 bits) | Register with base address (3 bits) | Offset from source register to be loaded from memory (6 bits)
    fn load_register(&mut self) {
        let instruction = self.current_instruction;
        //I "push" the bits for the register number to the rightmost position, and make all the other bits 0 by doing a bitwise AND with 0b111
        let destination_register = (instruction >> 9) & THREE_BIT_MASK;
        let source_address_register = (instruction >> 6) & THREE_BIT_MASK;
        let offset = extend_sign_for_integer(instruction & FIVE_BIT_MASK, 6);
        self.general_registers[destination_register as usize] = self
            .read_memory_and_check_keyboard_input(
                (self.general_registers[source_address_register as usize].wrapping_add(offset))
                    as usize,
            );
        self.update_flags(destination_register as usize);
    }

    /// Loads an address into a register. The address is an offset from from the program counter
    /// Structure: Opcode (4 bits) | Destination register number (3 bits) | Offset to be added (9 bits)
    fn load_address(&mut self) {
        let instruction = self.current_instruction;
        //I "push" the bits for the register number to the rightmost position, and make all the other bits 0 by doing a bitwise AND with 0b111
        let destination_register = (instruction >> 9) & THREE_BIT_MASK;
        let offset = extend_sign_for_integer(instruction & 0x1FF, 9);
        self.general_registers[destination_register as usize] =
            self.program_counter.wrapping_add(offset);
        self.update_flags(destination_register as usize);
    }

    /// Loads a value into a register from a location in memory. The address is an offset from the program counter
    /// Structure: Opcode (4bits) | Destination register number (3 bits) | Offset to be loaded from
    fn load_indirect(&mut self) {
        let instruction = self.current_instruction;
        //I "push" the bits for the register number to the rightmost position, and make all the other bits 0 by doing a bitwise AND with 0b111
        let destination_register = (instruction >> 9) & THREE_BIT_MASK;
        let offset = extend_sign_for_integer(instruction & 0x1FF, 9);
        let effective_address = self.read_memory_and_check_keyboard_input(
            self.program_counter.wrapping_add(offset) as usize,
        );
        self.general_registers[destination_register as usize] =
            self.read_memory_and_check_keyboard_input(effective_address as usize);
        self.update_flags(destination_register as usize);
    }

    /// Stores a value into memory from a register. The address is an offset from the program counter
    /// Structure: Opcode (4 bits) | Source register number (3 bits) | Offset from program counter to be loaded into memory (9 bits)
    fn store(&mut self) {
        let instruction = self.current_instruction;
        //I "push" the bits for the register number to the rightmost position, and make all the other bits 0 by doing a bitwise AND with 0b111
        let source_register = (instruction >> 9) & THREE_BIT_MASK;
        let offset = extend_sign_for_integer(instruction & 0x1FF, 9);
        self.memory[(self.program_counter.wrapping_add(offset)) as usize] =
            self.general_registers[source_register as usize];
    }

    /// Stores a value into memory from a register. The address is an offset from a register dictated by the instruction
    /// Structure: Opcode (4 bits) | Source register number (3 bits) | Register with base address (3 bits) | Offset from base register to be loaded into memory (6 bits)
    fn store_register(&mut self) {
        let instruction = self.current_instruction;
        //I "push" the bits for the register number to the rightmost position, and make all the other bits 0 by doing a bitwise AND with 0b111
        let source_register = (instruction >> 9) & THREE_BIT_MASK;
        let base_address_register = (instruction >> 6) & THREE_BIT_MASK;
        let offset = extend_sign_for_integer(instruction & SIX_BIT_MASK, 6);
        self.memory[(self.general_registers[base_address_register as usize].wrapping_add(offset))
            as usize] = self.general_registers[source_register as usize];
    }

    /// Stores an value from a register into the address pointed to by an address in memory, itself pointed to by the program counter + an offset
    /// Structure: Opcode (4 bits) | Source register (3 bits) | Offset of address from which to fetch destination (9 bits)
    fn store_indirect(&mut self) {
        let instruction = self.current_instruction;
        //I "push" the bits for the register number to the rightmost position, and make all the other bits 0 by doing a bitwise AND with 0b111
        let source_register = (instruction >> 9) & THREE_BIT_MASK;
        let offset = extend_sign_for_integer(instruction & NINE_BIT_MASK, 9);
        let address_to_store_in = self.read_memory_and_check_keyboard_input(
            (self.program_counter.wrapping_add(offset)) as usize,
        );
        self.memory[address_to_store_in as usize] =
            self.general_registers[source_register as usize];
    }

    /// Executes a trap routine
    /// The code for the trap rooutine is in the last 8 bits of the instruction
    fn execute_trap_routine(&mut self) {
        self.general_registers[R7] = self.program_counter;
        let instruction = self.current_instruction;
        let code_parse_result = TrapCode::try_from(instruction & EIGHT_BIT_MASK);
        let mut trap_code: TrapCode = TrapOUT;
        match code_parse_result {
            Ok(code) => {
                trap_code = code;
            }
            Err(error) => {
                self.handle_error(error);
            }
        }

        match trap_code {
            TrapPUTS => {
                self.puts();
            }
            TrapHALT => {
                self.halt();
            }
            TrapOUT => {
                self.out();
            }
            TrapIN => {
                self.trap_in();
            }
            TrapGETC => {
                self.get_character();
            }
            TrapPUTSP => {
                self.output_char8();
            }
        }
    }

    /// Outputs a string of characters, each one in a memory location
    /// Starts reading string from the address pointed to by R0, and stops when it encounters a null character
    fn puts(&self) {
        let mut character_to_output =
            (self.memory[self.general_registers[R0] as usize] & EIGHT_BIT_MASK) as u8 as char;
        let mut offset: usize = 0;
        while character_to_output != char::from(0x0) {
            print!("{}", character_to_output);
            offset += 1;
            character_to_output = (self.memory
                [(self.general_registers[R0] as usize).wrapping_add(offset)]
                & 0xFF) as u8 as char;
        }

        let Ok(_) = stdout().flush() else {
            panic!("Error writing to standard output in PUTS");
        };
    }

    /// Halts execution of the virtual machine, by changing running bit to zero
    fn halt(&mut self) {
        self.running = false;
    }

    /// Prints a single character, the address for which is contained in R0
    fn out(&mut self) {
        print!(
            "{}",
            (self.general_registers[R0] & EIGHT_BIT_MASK) as u8 as char
        );
        let Ok(_) = stdout().flush() else {
            panic!("Error writing to standard output in OUT");
        };
    }

    /// Takes a single character from stdin, prints it out on console and stores it in R0
    fn trap_in(&mut self) {
        println!("Enter a character: ");
        let mut character = [0; 1];
        let Ok(_) = stdin().read_exact(&mut character) else {
            panic!("Error reading from standard input in IN");
        };
        print!("{}", character[0]);
        let Ok(_) = stdout().flush() else {
            panic!("Error writing standard output in IN");
        };
        self.general_registers[R0] = character[0] as u16;
        self.update_flags(R0 as usize);
    }

    /// Takes a single character from stdin, and stores it in R0
    fn get_character(&mut self) {
        let mut character = [0; 1];
        let Ok(_) = stdin().read_exact(&mut character) else {
            panic!("Error reading from standard input in GETC");
        };
        self.general_registers[R0] = character[0] as u16;
        self.update_flags(0);
    }

    /// Output a string of characters, each represented as 8 bits, two per memory address
    /// Starts reading string from the address pointed to by R0, and stops when it encounters a null characte
    fn output_char8(&mut self) {
        let mut current_memory_value = self.memory[self.general_registers[R0] as usize];

        let mut character_to_output = (current_memory_value & EIGHT_BIT_MASK) as u8 as char;

        if character_to_output != char::from(0x0) {
            print!("{}", character_to_output);
            character_to_output = ((current_memory_value >> 8) & EIGHT_BIT_MASK) as u8 as char;
            let mut offset: usize = 1;
            while character_to_output != char::from(0x0) {
                print!("{}", character_to_output);
                current_memory_value =
                    self.memory[(self.general_registers[R0] as usize).wrapping_add(offset)];
                character_to_output = (current_memory_value & EIGHT_BIT_MASK) as u8 as char;
                if character_to_output == char::from(0x0) {
                    break;
                }
                print!("{}", character_to_output);
                offset += 1;
                character_to_output = ((current_memory_value >> 8) & EIGHT_BIT_MASK) as u8 as char;
            }
        }

        let Ok(_) = stdout().flush() else {
            panic!("Error writing to standard output in trap OUT.");
        };
    }

    fn read_image_file(&mut self, file_path: &Path) -> bool {
        let image_file = File::open(file_path).unwrap();
        let mut file_bytestream = image_file.bytes();

        let origin_address_byte_1 = file_bytestream.next().unwrap().unwrap();
        let origin_address_byte_2 = file_bytestream.next().unwrap().unwrap();

        let origin_address = u16::from_be_bytes([origin_address_byte_1, origin_address_byte_2]);
        let mut offset = 0;
        let maximum_offset = MAX_MEMORY_ADDRESS - origin_address as usize;

        while let Some(Ok(byte_1)) = file_bytestream.next() {
            let byte_2 = file_bytestream.next().unwrap().unwrap();
            self.memory[(origin_address.wrapping_add(offset)) as usize] =
                u16::from_be_bytes([byte_1, byte_2]);
            offset += 1;
            if offset == maximum_offset as u16 {
                break;
            }
        }
        true
    }
}

impl TryFrom<u16> for TrapCode {
    type Error = VMError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0x20 => Ok(TrapGETC),
            0x21 => Ok(TrapOUT),
            0x22 => Ok(TrapPUTS),
            0x23 => Ok(TrapIN),
            0x24 => Ok(TrapPUTSP),
            0x25 => Ok(TrapHALT),
            _ => Err(VMError::InvalidTrapCode),
        }
    }
}

impl TryFrom<u16> for OpCode {
    type Error = VMError;
    fn try_from(value: u16) -> Result<Self, Self::Error> {
        let value_opcode = value >> 12;
        match value_opcode {
            0b0000 => Ok(OpBR),
            0b0001 => Ok(OpADD),
            0b0010 => Ok(OpLD),
            0b0011 => Ok(OpST),
            0b0100 => Ok(OpJSR),
            0b0101 => Ok(OpAND),
            0b0110 => Ok(OpLDR),
            0b0111 => Ok(OpSTR),
            0b1001 => Ok(OpNOT),
            0b1010 => Ok(OpLDI),
            0b1011 => Ok(OpSTI),
            0b1100 => Ok(OpJMP),
            0b1110 => Ok(OpLEA),
            0b1111 => Ok(OpTRAP),
            _ => Err(VMError::InvalidOpCode),
        }
    }
}

impl BitAnd<Flag> for u16 {
    type Output = u16;

    fn bitand(self, condition_flag: Flag) -> Self::Output {
        self & condition_flag as u16
    }
}

impl BitOr<Flag> for u16 {
    type Output = u16;

    fn bitor(self, condition_flag: Flag) -> Self::Output {
        self | condition_flag as u16
    }
}

impl BitOrAssign<Flag> for u16 {
    fn bitor_assign(&mut self, condition_flag: Flag) {
        *self = *self | condition_flag as u16;
    }
}

//I implement indexing arrays with register enum values to make code more declarative
impl<T> Index<GeneralPurposeRegister> for [T] {
    type Output = T;
    fn index(&self, idx: GeneralPurposeRegister) -> &Self::Output {
        match idx {
            GeneralPurposeRegister::R0 => &self[0],
            GeneralPurposeRegister::R1 => &self[1],
            GeneralPurposeRegister::R2 => &self[2],
            GeneralPurposeRegister::R3 => &self[3],
            GeneralPurposeRegister::R4 => &self[4],
            GeneralPurposeRegister::R5 => &self[5],
            GeneralPurposeRegister::R6 => &self[6],
            GeneralPurposeRegister::R7 => &self[7],
        }
    }
}

impl<T> IndexMut<GeneralPurposeRegister> for [T] {
    fn index_mut(&mut self, idx: GeneralPurposeRegister) -> &mut Self::Output {
        match idx {
            GeneralPurposeRegister::R0 => &mut self[0],
            GeneralPurposeRegister::R1 => &mut self[1],
            GeneralPurposeRegister::R2 => &mut self[2],
            GeneralPurposeRegister::R3 => &mut self[3],
            GeneralPurposeRegister::R4 => &mut self[4],
            GeneralPurposeRegister::R5 => &mut self[5],
            GeneralPurposeRegister::R6 => &mut self[6],
            GeneralPurposeRegister::R7 => &mut self[7],
        }
    }
}

fn main() {
    let mut vm = LC3VM::new();
    if let Ok(mut original_tio) = Termios::from_fd(stdin().as_raw_fd()) {
        ctrlc::set_handler(move || {
            handle_interrupt(&mut original_tio);
        })
        .expect("Error setting Ctrl-C handler");

        if let Err(error) = disable_input_buffering(&mut original_tio) {
            vm.handle_error(error);
        }

        let args: Vec<String> = env::args().collect();

        if args.len() < 2 {
            /* show usage string */
            println!("Usage: cargo run [image path 1] [image path 2]...");
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

#[cfg(test)]
mod tests {
    use super::Flag::*;
    use super::GeneralPurposeRegister::*;
    use super::*;

    #[test]
    fn add_writes_value_in_register_correctly_for_2_register_add() {
        let mut vm = LC3VM::new();

        vm.general_registers[R0] = 5;
        vm.general_registers[R1] = 4;

        let add_instruction =
            (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | R0 as u16;

        vm.current_instruction = add_instruction;

        vm.add();

        assert_eq!(vm.general_registers[R2], 9);
    }

    #[test]
    fn add_writes_value_in_register_correctly_for_register_and_immediate() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 5;

        let add_instruction =
            (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 4;

        vm.current_instruction = add_instruction;

        vm.add();

        assert_eq!(vm.general_registers[R2], 9);
    }

    #[test]
    fn add_does_subtraction_correctly_for_immediate() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 5;

        let add_instruction =
            (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 011011; // 0b11111011 is -5 in two's complement

        vm.current_instruction = add_instruction;

        vm.add();

        assert_eq!(vm.general_registers[R2] as i16, 0);
    }

    #[test]
    fn add_does_subtraction_correctly_for_second_register() {
        let mut vm = LC3VM::new();

        vm.general_registers[R0] = 5;
        vm.general_registers[R1] = 0b1111111111111011; //-6 in two's complement

        let add_instruction =
            (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | R0 as u16;
        vm.current_instruction = add_instruction;

        vm.add();

        assert_eq!(vm.general_registers[R2] as i16, 0);
    }

    #[test]
    fn add_updates_neg_flag_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 5;

        let add_instruction =
            (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 0b11010; // 0b11010 is -6 in two's complement with 5 bits
        vm.current_instruction = add_instruction;

        vm.add();

        assert!(vm.condition_flags & Negative != 0);
    }

    #[test]
    fn add_updates_zero_flag_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 5;

        let add_instruction =
            (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 0b11011; // 0b11011 is -5 in two's complement for a 5 bit value

        vm.current_instruction = add_instruction;

        vm.add();

        assert!(vm.condition_flags & Zero != 0);
    }

    #[test]
    fn add_updates_pos_flag_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 5;

        let add_instruction =
            (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 0b1;

        vm.current_instruction = add_instruction;

        vm.add();

        assert!(vm.condition_flags & Positive != 0);
    }

    #[test]
    fn and_writes_value_in_register_correctly_for_register_and_immediate() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 0b101;

        let and_instruction =
            (OpCode::OpAND as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 0b01001;

        vm.current_instruction = and_instruction;

        vm.and();

        assert_eq!(vm.general_registers[R2], 0b00001);
    }

    #[test]
    fn and_writes_value_in_register_correctly_for_two_registers() {
        let mut vm = LC3VM::new();

        vm.general_registers[R0] = 0b1001;
        vm.general_registers[R1] = 0b101;

        let and_instruction =
            (OpCode::OpAND as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | R0 as u16;

        vm.current_instruction = and_instruction;

        vm.and();

        assert_eq!(vm.general_registers[R2], 0b00001);
    }

    #[test]
    fn and_updates_neg_flag_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 0xF000;

        let and_instruction =
            (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 0b11010; // 0b11111010 is -6 in two's complement

        vm.current_instruction = and_instruction;

        vm.and();

        assert!(vm.condition_flags & Negative != 0);
    }

    #[test]
    fn and_updates_zero_flag_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 0;

        let and_instruction =
            (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 0b00110; // 0b11011 is -5 in two's complement for a 5 bit value

        vm.current_instruction = and_instruction;

        vm.and();

        assert!(vm.condition_flags & Zero != 0);
    }

    #[test]
    fn and_updates_pos_flag_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 5;

        let and_instruction =
            (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 0b1;

        vm.current_instruction = and_instruction;

        vm.and();

        assert!(vm.condition_flags & Positive != 0);
    }

    #[test]
    fn not_writes_value_in_register_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R0] = 0xFFF0 | 0b1001;

        let not_instruction = (OpCode::OpAND as u16) | ((R1 as u16) << 9) | ((R0 as u16) << 6);

        vm.current_instruction = not_instruction;

        vm.not();

        assert_eq!(vm.general_registers[R1], (0x0000 | 0b0110));
    }

    #[test]
    fn not_updates_neg_flag_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R0] = 0;

        let not_instruction = (OpCode::OpADD as u16) | ((R1 as u16) << 9) | ((R0 as u16) << 6);

        vm.current_instruction = not_instruction;

        vm.not();

        assert!(vm.condition_flags & Negative != 0);
    }

    #[test]
    fn not_updates_zero_flag_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R0] = 0xFFFF;

        let not_instruction = (OpCode::OpADD as u16) | ((R1 as u16) << 9) | ((R0 as u16) << 6);

        vm.current_instruction = not_instruction;

        vm.not();

        assert!(vm.condition_flags & Zero != 0);
    }

    #[test]
    fn not_updates_pos_flag_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R0] = 0xF000;

        let not_instruction = (OpCode::OpADD as u16) | ((R1 as u16) << 9) | ((R0 as u16) << 6);

        vm.current_instruction = not_instruction;

        vm.not();

        assert!(vm.condition_flags & Positive != 0);
    }

    #[test]
    fn branch_instruction_branches_for_neg() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 5;

        let add_instruction =
            (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 0b11010; // 0b11010 is -6 in two's complement with 5 bits

        vm.current_instruction = add_instruction;

        vm.add();

        let branch_instruction = (OpCode::OpBR as u16) | (0b100 << 9) | 2;

        vm.current_instruction = branch_instruction;

        vm.branch();

        assert_eq!(vm.program_counter, 0x3002);
    }

    #[test]
    fn branch_instruction_branches_for_zero() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 6;

        let add_instruction =
            (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 0b11010; // 0b11010 is -6 in two's complement with 5 bits

        vm.current_instruction = add_instruction;

        vm.add();

        let branch_instruction = (OpCode::OpBR as u16) | (0b010 << 9) | 2;

        vm.current_instruction = branch_instruction;

        vm.branch();

        assert_eq!(vm.program_counter, 0x3002);
    }

    #[test]
    fn branch_instruction_branches_for_positive() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 7;

        let add_instruction =
            (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 0b11010; // 0b11010 is -6 in two's complement with 5 bits

        vm.current_instruction = add_instruction;

        vm.add();

        let branch_instruction = (OpCode::OpBR as u16) | (0b001 << 9) | 2;

        vm.current_instruction = branch_instruction;

        vm.branch();

        assert_eq!(vm.program_counter, 0x3002);
    }

    #[test]
    fn branch_instruction_branches_for_positive_or_zero() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 7;

        let add_instruction =
            (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 0b11010; // 0b11010 is -6 in two's complement with 5 bits

        vm.current_instruction = add_instruction;

        vm.add();

        let branch_instruction = (OpCode::OpBR as u16) | (0b011 << 9) | 2;

        vm.current_instruction = branch_instruction;

        vm.branch();

        assert_eq!(vm.program_counter, 0x3002);
    }

    #[test]
    fn branch_instruction_branches_for_negative_or_zero() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 5;

        let add_instruction =
            (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 0b11010; // 0b11010 is -6 in two's complement with 5 bits

        vm.current_instruction = add_instruction;

        vm.add();

        let branch_instruction = (OpCode::OpBR as u16) | (0b101 << 9) | 2;

        vm.current_instruction = branch_instruction;

        vm.branch();

        assert_eq!(vm.program_counter, 0x3002);
    }

    #[test]
    fn branch_instruction_doesnt_branch_with_no_flags() {
        let mut vm = LC3VM::new();

        let branch_instruction = (OpCode::OpBR as u16) | (0b001 << 9) | 2;

        vm.current_instruction = branch_instruction;

        vm.branch();

        assert_eq!(vm.program_counter, 0x3000);
    }

    #[test]
    fn jump_instruction_works_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R2] = 2;

        let jump_instruction = (OpCode::OpJMP as u16) | (0b000 << 9) | ((R2 as u16) << 6);

        vm.current_instruction = jump_instruction;

        vm.jump();

        assert_eq!(vm.program_counter, 2);
    }

    #[test]
    fn jsr_works_correctly_for_offset_mode() {
        let mut vm = LC3VM::new();

        vm.general_registers[R2] = 2;
        vm.program_counter = 10;

        let jump_instruction = (OpCode::OpJSR as u16) | (0b1 << 11) | 15;

        vm.current_instruction = jump_instruction;

        vm.jump_register_or_offset();

        assert_eq!(vm.program_counter, 25);
        assert_eq!(vm.general_registers[R7], 10);
    }

    #[test]
    fn jsr_works_correctly_for_register_mode() {
        let mut vm = LC3VM::new();

        vm.general_registers[R2] = 2;
        vm.program_counter = 10;

        let jump_instruction = (OpCode::OpJSR as u16) | ((R2 as u16) << 6);

        vm.current_instruction = jump_instruction;

        vm.jump_register_or_offset();

        assert_eq!(vm.program_counter, 2);
        assert_eq!(vm.general_registers[R7], 10);
    }

    #[test]
    fn load_works_correctly() {
        let mut vm = LC3VM::new();

        vm.memory[42] = 42;
        vm.program_counter = 10;

        let load_instruction = (OpCode::OpLD as u16) | ((R0 as u16) << 9) | 32;

        vm.current_instruction = load_instruction;

        vm.load();

        assert_eq!(vm.program_counter, 10);
        assert_eq!(vm.general_registers[R0], 42);
    }

    #[test]
    fn load_register_works_correctly() {
        let mut vm = LC3VM::new();

        vm.memory[42] = 42;
        vm.program_counter = 10;
        vm.general_registers[R1] = 11;

        let load_register_instruction =
            (OpCode::OpLDR as u16) | ((R0 as u16) << 9) | ((R1 as u16) << 6) | 31;

        vm.current_instruction = load_register_instruction;

        vm.load_register();

        assert_eq!(vm.general_registers[R0], 42);
    }

    #[test]
    fn load_address_works_correctly() {
        let mut vm = LC3VM::new();

        vm.program_counter = 10;

        let load_address_instruction = (OpCode::OpLEA as u16) | ((R0 as u16) << 9) | 32;

        vm.current_instruction = load_address_instruction;

        vm.load_address();

        assert_eq!(vm.program_counter, 10);
        assert_eq!(vm.general_registers[R0], 42);
    }

    #[test]
    fn load_updates_flags() {
        let mut vm = LC3VM::new();

        vm.memory[42] = 42;
        vm.program_counter = 10;

        let load_instruction = (OpCode::OpLD as u16) | ((R0 as u16) << 9) | 32;

        vm.current_instruction = load_instruction;

        vm.load();

        assert!(vm.condition_flags & Positive != 0);

        vm.memory[42] = 0;
        vm.program_counter = 10;

        let load_instruction = (OpCode::OpLD as u16) | ((R0 as u16) << 9) | 32;

        vm.current_instruction = load_instruction;

        vm.load();

        assert!(vm.condition_flags & Zero != 0);

        vm.memory[42] = 0xF000;
        vm.program_counter = 10;

        let load_instruction = (OpCode::OpLD as u16) | ((R0 as u16) << 9) | 32;

        vm.current_instruction = load_instruction;

        vm.load();

        assert!(vm.condition_flags & Negative != 0);
    }

    #[test]
    fn load_register_updates_flags() {
        let mut vm = LC3VM::new();

        vm.memory[42] = 42;
        vm.program_counter = 10;
        vm.general_registers[R1] = 11;

        let load_register_instruction =
            (OpCode::OpLDR as u16) | ((R0 as u16) << 9) | ((R1 as u16) << 6) | 31;

        vm.current_instruction = load_register_instruction;

        vm.load_register();

        assert!(vm.condition_flags & Positive != 0);

        vm.memory[42] = 0xF000;
        vm.program_counter = 10;
        vm.general_registers[R1] = 11;

        let load_register_instruction =
            (OpCode::OpLDR as u16) | ((R0 as u16) << 9) | ((R1 as u16) << 6) | 31;

        vm.current_instruction = load_register_instruction;

        vm.load_register();

        assert!(vm.condition_flags & Negative != 0);

        vm.memory[42] = 0;
        vm.program_counter = 10;

        let load_instruction = (OpCode::OpLD as u16) | ((R0 as u16) << 9) | 32;

        vm.current_instruction = load_register_instruction;

        vm.load();

        assert!(vm.condition_flags & Zero != 0);
    }

    #[test]
    fn load_address_updates_flags() {
        let mut vm = LC3VM::new();

        vm.program_counter = 10;

        let load_address_instruction = (OpCode::OpLEA as u16) | ((R0 as u16) << 9) | 32;

        vm.current_instruction = load_address_instruction;

        vm.load_address();

        assert!(vm.condition_flags & Positive != 0);

        vm.program_counter = 5;

        let load_address_instruction = (OpCode::OpLEA as u16) | ((R0 as u16) << 9) | 0b111111010; // 0b111111010 is -6 in two's complement with 9 bits

        vm.current_instruction = load_address_instruction;

        vm.load_address();

        assert!(vm.condition_flags & Negative != 0);

        vm.program_counter = 6;

        let load_address_instruction = (OpCode::OpLEA as u16) | ((R0 as u16) << 9) | 0b111111010; // 0b11010 is -6 in two's complement with 9 bits

        vm.current_instruction = load_address_instruction;

        vm.load_address();

        assert!(vm.condition_flags & Zero != 0);
    }

    #[test]
    fn load_indirect_works_correctly() {
        let mut vm = LC3VM::new();

        vm.program_counter = 10;

        let load_indirect_instruction = (OpCode::OpLDI as u16) | ((R0 as u16) << 9) | 32;

        vm.memory[42] = 43;
        vm.memory[43] = 44;

        vm.current_instruction = load_indirect_instruction;

        vm.load_indirect();

        assert_eq!(vm.general_registers[R0], 44);

        vm.program_counter = 10;

        vm.memory[42] = 43;
        vm.memory[43] = 0xF000;

        let load_indirect_instruction = (OpCode::OpLDI as u16) | ((R0 as u16) << 9) | 32; // 0b111111010 is -6 in two's complement with 9 bits

        vm.current_instruction = load_indirect_instruction;

        vm.load_indirect();

        assert_eq!(vm.general_registers[R0], 0xF000);

        vm.program_counter = 10;

        let load_address_instruction = (OpCode::OpLDI as u16) | ((R0 as u16) << 9) | 32; // 0b11010 is -6 in two's complement with 9 bits

        vm.memory[42] = 43;
        vm.memory[43] = 0;

        vm.current_instruction = load_address_instruction;

        vm.load_indirect();

        assert_eq!(vm.general_registers[R0], 0);
    }

    #[test]
    fn load_indirect_updates_flags_correctly() {
        let mut vm = LC3VM::new();

        vm.program_counter = 10;

        let load_indirect_instruction = (OpCode::OpLDI as u16) | ((R0 as u16) << 9) | 32;

        vm.memory[42] = 43;
        vm.memory[43] = 44;

        vm.current_instruction = load_indirect_instruction;

        vm.load_indirect();

        assert!(vm.condition_flags & Positive != 0);

        vm.program_counter = 10;

        vm.memory[42] = 43;
        vm.memory[43] = 0xF000;

        let load_indirect_instruction = (OpCode::OpLDI as u16) | ((R0 as u16) << 9) | 32; // 0b111111010 is -6 in two's complement with 9 bits

        vm.current_instruction = load_indirect_instruction;

        vm.load_indirect();

        assert!(vm.condition_flags & Negative != 0);

        vm.program_counter = 10;

        let load_address_instruction = (OpCode::OpLDI as u16) | ((R0 as u16) << 9) | 32; // 0b11010 is -6 in two's complement with 9 bits

        vm.memory[42] = 43;
        vm.memory[43] = 0;

        vm.current_instruction = load_address_instruction;

        vm.load_indirect();

        assert!(vm.condition_flags & Zero != 0);
    }

    #[test]
    fn store_works_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R0] = 42;
        vm.program_counter = 10;

        let store_instruction = (OpCode::OpST as u16) | ((R0 as u16) << 9) | 32;

        vm.current_instruction = store_instruction;

        vm.store();

        assert_eq!(vm.program_counter, 10);
        assert_eq!(vm.memory[42], 42);
    }

    #[test]
    fn store_register_works_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R0] = 42;
        vm.general_registers[R1] = 11;

        let store_instruction =
            (OpCode::OpST as u16) | ((R0 as u16) << 9) | ((R1 as u16) << 6) | 31;

        vm.memory[vm.program_counter as usize] = store_instruction;

        vm.store_register();

        assert_eq!(vm.memory[42], 42);
    }

    #[test]
    fn store_indirect_works_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R0] = 42;
        vm.program_counter = 10;
        vm.memory[20] = 42;

        let store_instruction = (OpCode::OpST as u16) | ((R0 as u16) << 9) | 10;

        vm.current_instruction = store_instruction;

        vm.store_indirect();

        assert_eq!(vm.memory[42], 42);
    }

    #[test]
    fn puts_displays_string_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R0] = 40;
        vm.memory[40] = 'T' as u16;
        vm.memory[41] = 'e' as u16;
        vm.memory[42] = 's' as u16;
        vm.memory[43] = 't' as u16;
        vm.memory[44] = '_' as u16;
        vm.memory[45] = 'O' as u16;
        vm.memory[46] = 'K' as u16;

        let trap_puts_instruction = OpCode::OpTRAP as u16 | TrapPUTS as u16;

        vm.current_instruction = trap_puts_instruction;

        vm.execute_trap_routine();
    }

    #[test]
    fn halt_stops_execution() {
        let mut vm = LC3VM::new();

        let trap_halt_instruction = OpCode::OpTRAP as u16 | TrapHALT as u16;

        vm.current_instruction = trap_halt_instruction;

        vm.execute_trap_routine();

        assert!(!vm.running);
    }

    #[test]
    fn out_outputs_char() {
        let mut vm = LC3VM::new();

        let trap_out_instruction = OpCode::OpTRAP as u16 | TrapOUT as u16;

        vm.general_registers[R0] = 'T' as u16;

        vm.current_instruction = trap_out_instruction;

        vm.execute_trap_routine();

        vm.general_registers[R0] = 'e' as u16;

        vm.execute_trap_routine();

        vm.general_registers[R0] = 's' as u16;

        vm.execute_trap_routine();

        vm.general_registers[R0] = 't' as u16;

        vm.execute_trap_routine();

        vm.general_registers[R0] = '_' as u16;

        vm.execute_trap_routine();

        vm.general_registers[R0] = 'O' as u16;

        vm.execute_trap_routine();

        vm.general_registers[R0] = 'K' as u16;

        vm.current_instruction = trap_out_instruction;

        vm.execute_trap_routine();
    }

    /*#[test]
    fn in_works_correctly() {
        let mut vm = LC3VM::new();

        let trap_in_instruction = OpCode::OpTRAP as u16 | TrapIN as u16;

        vm.execute_trap_routine();

        assert_eq!(vm.general_registers[R0], 'R' as u16);
    }

    #[test]
    fn get_character_works_correctly() {
        let mut vm = LC3VM::new();

        let trap_in_instruction = OpCode::OpTRAP as u16 | TrapGETC as u16;

        vm.execute_trap_routine();

        assert_eq!(vm.general_registers[R0], 'R' as u16);
    } */

    #[test]
    fn putsp_displays_string_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R0] = 40;
        vm.memory[40] = 'T' as u16 | (('e' as u16) << 8);
        vm.memory[41] = 's' as u16 | (('t' as u16) << 8);
        vm.memory[42] = '_' as u16 | (('O' as u16) << 8);
        vm.memory[43] = 'K' as u16;

        let trap_putsp_instruction = OpCode::OpTRAP as u16 | TrapPUTSP as u16;

        vm.current_instruction = trap_putsp_instruction;

        vm.execute_trap_routine();
    }

    #[test]
    fn binary_file_is_read_correctly() {
        let mut vm = LC3VM::new();
        vm.program_counter = 0;
        vm.read_image_file(&Path::new("./binaries/test.obj"));
        let mut text_array = ['a'; 4];
        text_array[0] = (vm.memory[42] >> 8) as u8 as char;
        text_array[1] = (vm.memory[42] & 0xFF) as u8 as char;
        text_array[2] = (vm.memory[43] >> 8) as u8 as char;
        text_array[3] = (vm.memory[43] & 0xFF) as u8 as char;

        assert_eq!(text_array[0], 'T');
        assert_eq!(text_array[1], 'e');
        assert_eq!(text_array[2], 's');
        assert_eq!(text_array[3], 't');
    }

    #[test]
    fn invalid_opcode_is_handled() {
        let mut vm = LC3VM::new();

        let invalid_instruction = 0b1000 << 12;

        vm.memory[vm.program_counter as usize] = invalid_instruction;
        vm.decode_instruction();

        assert!(!vm.running);
    }

    #[test]
    fn invalid_trapcode_is_handled() {
        let mut vm = LC3VM::new();

        let invalid_instruction = 0b1111 << 12 | 0x19;

        vm.memory[vm.program_counter as usize] = invalid_instruction;
        let code: OpCode = vm.decode_instruction().unwrap();
        vm.execute_instruction(code);

        assert!(!vm.running);
    }
}
