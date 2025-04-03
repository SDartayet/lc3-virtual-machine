use std::{ops::{Index, IndexMut}, slice::SliceIndex};

//The number of memory addresses is 2^16
const MAX_MEMORY_ADDRESS: u16 = u16::MAX;
const REGISTER_COUNT: u8 = 10;

type lc3_instruction = u16;

///The registers the architecture contains.
/// R0 to R7 are the general purpose registers. PC is the instruction pointer.
/// RCOND is the flags register,
enum Register {
    R0,
    R1,
    R2,
    R3,
    R4,
    R5,
    R6,
    R7,
    PC, /* program counter */
    RCOND,
}

struct LC3VM {
    general_registers: [u16;8],
    program_counter: u16,
    condition_flags: [u16;3],
    memory: [u16;MAX_MEMORY_ADDRESS as usize]
}

impl LC3VM {

    fn new() -> LC3VM {
        LC3VM { general_registers: [0;8], program_counter: 0, condition_flags: 0, memory: [0;MAX_MEMORY_ADDRESS as usize] }
    }

    /// Updates the condition flags according to the result of the last arithmetic operations
    /// Input: the register on which the result was stored
    fn update_flags(&mut self, register_number: usize) {
        let register_value = self.general_registers[register_number];
        if register_value == 0 {
            self.condition_flags[Flag::FlZero] = 1;
        } else if register_value & 0x8000 != 0 {
            self.condition_flags[Flag::FlNeg] = 1;
        } else {
            self.condition_flags[Flag::FlPos] = 1;
        }
    }

    /// Implements the addition instruciton
    /// Add instruction structure: OPCODE (4 bits) | Destination register (3 bits) | Source register 1 | Mode (immediate or another register) | 00 Source register 2 (3 bits) on mode 0 or a 5 bit value on mode 1
    fn add(&mut self, instruction: lc3_instruction) {
        //I "push" the bits for the register number to the rightmost position, and make all the other bits 0 by doing a bitwise AND with 0b111
        let source_register_1_number= ((instruction >> 6) & 0b111) as usize; 

        //Same as previously, but I need to shift right by 9 bits given there are 9 bits befrore the destination register
        let destination_register_number = ((instruction >> 9) & 0b111) as usize; 
        
        //Mode flag is just one bit, so the bitwise AND is with 0b1. If it's zero I use register mode, if one immediate mode
        let mode_flag = (instruction >> 5) & 0b1 == 0;

        let second_operand: u16;
        
        if mode_flag {
            let source_register_2_number = (instruction & 0b111) as usize;
            second_operand = self.general_registers[source_register_2_number];
        } else {
            second_operand = instruction & 0b11111;
        }
        self.general_registers[destination_register_number] = self.general_registers[source_register_1_number] + second_operand;

        self.update_flags(destination_register_number);

    }
}

/// The opcodes for the instructions the architecture supports
enum OpCode {
    OpBR,   /* branch */
    OpADD,  /* add  */
    OpLD,   /* load */
    OpST,   /* store */
    OpJSR,  /* jump register */
    OpAND,  /* bitwise and */
    OpLDR,  /* load register */
    OpSTR,  /* store register */
    OpRTI,  /* unused */
    OpNOT,  /* bitwise not */
    OpLDI,  /* load indirect */
    OpSTI,  /* store indirect */
    OpJMP,  /* jump */
    OpRES,  /* reserved (unused) */
    OpLEA,  /* load effective address */
    OpTRAP, /* execute trap */
}

enum Flag {
    FlPos,  /* Set when the result of the previous operation was positive */
    FlZero, /* Set when the result of the previous operation was zero */
    FlNeg,  /* Set when the result of the previous operation was negative */
}


impl<T> Index<Flag> for [T] {
    type Output = T;
    fn index(&self, idx: Flag) -> &Self::Output {
        match idx {
            Flag::FlPos => &self[0b1],
            Flag::FlZero => &self[0b10],
            Flag::FlNeg => &self[0b100],
        }
    }
}

impl<T> IndexMut<Flag> for [T] {
    fn index_mut(&mut self, idx: Flag) -> &mut Self::Output {
        match idx {
            Flag::FlPos => &mut self[0b1],
            Flag::FlZero => &mut self[0b10],
            Flag::FlNeg => &mut self[0b100],
        }
    }
}

fn main() {
    println!("Hello, world!");
}
