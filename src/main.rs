use std::{
    ops::{Index, IndexMut, Shl},
    slice::SliceIndex,
};

//The number of memory addresses is 2^16
const MAX_MEMORY_ADDRESS: u16 = u16::MAX;
const REGISTER_COUNT: u8 = 10;

type lc3_instruction = u16;

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

fn extend_sign_for_integer(value: u16, value_bit_count: u16) -> u16 {
    // If the first bit of the value is negative, because of how two's complement works, we need to extend it with ones unti lwe have 16 bits to preserve the sign
    // I check if the first bit of the value is one, and if it is I extend it with ones, otherwise with zeroes
    let is_immediate_negative = (value & (1 << value_bit_count - 1)) != 0;
    // If the sign extension is negative, I fill with 11 bits of 1, since 11+5 = 16. Otherwise I don't need to do anything because the filler would be zeroes
    if is_immediate_negative {
        // Doing a bitwise or between the sign extension I need and the immediate value I get the immediate with the sign extended
        // The sign extension will be a series of ones left of an amount of zeroes equivalent to the bits of my current value
        value | (0xFFFF << value_bit_count)
    } else { value }
}
struct LC3VM {
    general_registers: [u16; 8],
    program_counter: u16,
    condition_flags: [u16; 3],
    memory: [u16; MAX_MEMORY_ADDRESS as usize],
}
use Flag::{*}; 

impl LC3VM {
    

    fn new() -> LC3VM {
        LC3VM {
            general_registers: [0; 8],
            program_counter: 0,
            condition_flags: [0; 3],
            memory: [0; MAX_MEMORY_ADDRESS as usize],
        }
    }

    /// Updates the condition flags according to the result of the last arithmetic operations
    /// Input: the register on which the result was stored
    fn update_flags(&mut self, register_number: usize) {
        //I reset the condition flags first
        self.condition_flags = [0;3];
        let register_value = self.general_registers[register_number];
        if register_value == 0 {
            self.condition_flags[FlZero] = 1;
        } else if register_value & (1 << 15) != 0 {
            //In two's complement, if the first bit is one, the number is negative
            self.condition_flags[FlNeg] = 1;
        } else {
            self.condition_flags[FlPos] = 1;
        }
    }

    /// Implements the addition instruciton
    /// Instruction structure: OPCODE (4 bits) | Destination register (3 bits) | Source register 1 (3 bits) | Mode (immediate or another register, 1 bit) | [00 | Source register 2 (3 bits)] on mode 0 or a 5 bit value on mode 1
    fn add(&mut self, instruction: lc3_instruction) {
        //I "push" the bits for the register number to the rightmost position, and make all the other bits 0 by doing a bitwise AND with 0b111
        let source_register_1_number = ((instruction >> 6) & 0b111) as usize;

        //Same as previously, but I need to shift right by 9 bits given there are 9 bits befrore the destination register
        let destination_register_number = ((instruction >> 9) & 0b111) as usize;

        //Mode flag is just one bit, so the bitwise AND is with 0b1. If it's zero I use register mode, if one immediate mode
        let mode_flag = (instruction >> 5) & 0b1 == 0;

        let second_operand: u16;

        if mode_flag {
            // I get the number of the second register from the last 3 bits
            let source_register_2_number = (instruction & 0b111) as usize;
            second_operand = self.general_registers[source_register_2_number];
        } else {
            let five_bit_immediate = instruction & 0b11111; // I filter the first 5 bits of the instruction, which contain the immediate, and set the rest to zero
            second_operand = extend_sign_for_integer(five_bit_immediate, 5);
        }

        //Wrapping add lets us recreate the way addition works in two's complement systems while keeping the values unsigned (for more generalization)
        self.general_registers[destination_register_number] =
            self.general_registers[source_register_1_number].wrapping_add(second_operand);

        self.update_flags(destination_register_number);
    }

    /// Implements the bitwise NOT instruction
    /// Instruction structure: OPCODE (4 bits) | Destination register (3 bits) | Source register (3 bits) | 111111
    fn not(&mut self, instruction: lc3_instruction) {
        //I "push" the bits for the register number to the rightmost position, and make all the other bits 0 by doing a bitwise AND with 0b111
        let source_register_number = ((instruction >> 6) & 0b111) as usize;

        //Same as previously, but I need to shift right by 9 bits given there are 9 bits befrore the destination register
        let destination_register_number = ((instruction >> 9) & 0b111) as usize;

        self.general_registers[destination_register_number] =
            !self.general_registers[source_register_number];

        self.update_flags(destination_register_number);
    }

    /// Implements the bitwise AND instruction
    /// Instruction structure: OPCODE (4 bits) | Destination register (3 bits) | Source register 1 (3 bits) | Mode (immediate or another register, 1 bit) | [00 Source register 2 (3 bits)] on mode 0 or a 5 bit value on mode 1
    fn and(&mut self, instruction: lc3_instruction) {
        //I "push" the bits for the register number to the rightmost position, and make all the other bits 0 by doing a bitwise AND with 0b111
        let source_register_1_number = ((instruction >> 6) & 0b111) as usize;

        //Same as previously, but I need to shift right by 9 bits given there are 9 bits befrore the destination register
        let destination_register_number = ((instruction >> 9) & 0b111) as usize;

        //Mode flag is just one bit, so the bitwise AND is with 0b1. If it's zero I use register mode, if one immediate mode
        let mode_flag = (instruction >> 5) & 0b1 == 0;

        let second_operand: u16;

        if mode_flag {
            let source_register_2_number = (instruction & 0b111) as usize;
            second_operand = self.general_registers[source_register_2_number];
        } else {
            // If the first bit of the immediate value is negative, because of how two's complement works, we need to extend it with ones until we have 16 bits to preserve the sign
            // I check if the first bit of the 5 bit immediate is one, and if it is I extend it with ones, otherwise with zeroes
            let five_bit_immediate = instruction & 0b11111; // I filter the first 5 bits of the instruction, which contain the immediate, and set the rest to zero
            second_operand = extend_sign_for_integer(five_bit_immediate, 5);
        }
        self.general_registers[destination_register_number] =
            self.general_registers[source_register_1_number] & second_operand;

        self.update_flags(destination_register_number);
    }

    /// Jumps a set number of instructions if zero, negative or positive flags are up (depending on encoding)
    /// Instruction structure: OPCODE (4 bits) | Negative flag (1 bit) | Zero flag (1 bit) | Positive flag (1 bit) | Offset from current PC value to which to jump (9 bits)
    fn branch(&mut self, instruction: lc3_instruction) {
        //The 9 rightmost bits contain the offset I need to jump when the condition is true
        //ox1FF is 9 bits set to 1
        let program_counter_offset = instruction & 0x01FF;

        //Starting left, bit 10 is that the condition is the positive flag being up, bit 11 is the zero one being up, and bit 12 the negative one
        //I shift 9 rightward and and the value with 0b111 to get the value of the three
        let flag_values =(instruction >> 9) & 0b111;
        let condition_flag_is_up: bool; 
        if flag_values == 0 {
            condition_flag_is_up = false;
        } else {
            condition_flag_is_up = self.condition_flags[Flag::from(flag_values)] != 0;
        }
        
        if condition_flag_is_up {
            self.program_counter += program_counter_offset;
        }
    }

    /// Makes PC jump to the memory address in the register indicated by the instruction
    /// Instruction structure: OPCODE (4 bits) | 000 | Number of the register with the memory address (3 bits) | 000000
    fn jump(&mut self, instruction: lc3_instruction) {
        ///I get the destination register by skipping the filler zeroes and getting the three bits that come after that
        let destination_register = ((instruction >> 6) & 0b111) as usize;
        self.program_counter = self.general_registers[destination_register];
    }

    /// Makes PC jump to the memory address in the register indicated by the instruction or to an offset, depending on the mode
    /// Instruction structure: OPCODE (4 bits) | Mode (0 for register and 1 for offset, 1 bit) | [00 | Number of the register with the memory address (3 bits) | 000000] in register mode or 11 bit offset in offset mode
    fn jump_register_or_offset(&mut self, instruction: lc3_instruction) {
        // I get the 11th bit. If it's zero then I need to use register mode, if it's one I need to use offset mode
        let is_register_mode = (instruction >> 11) == 0;

        if (is_register_mode) {
            ///I get the destination register by skipping the filler zeroes and getting the three bits that come after that
            let destination_register = ((instruction >> 6) & 0b111) as usize;
            self.program_counter = self.general_registers[destination_register];
        } else {
            let offset = instruction & 0x7FF; // 0x7FF consists of 11 bits of ones; I want to get the rightmost 11 bits which contain the offset
            extend_sign_for_integer(offset, 11);
            self.program_counter += offset;
        }

        ///I get the destination register by skipping the filler zeroes and getting the three bits that come after that
        let destination_register = ((instruction >> 6) & 0b111) as usize;
        self.program_counter = self.general_registers[destination_register];
    }
}

/// The opcodes for the instructions the architecture supports
enum OpCode {
    OpBR = 0000 << 12,    /* branch */
    OpADD = 0b0001 << 12, /* add  */
    OpLD,                 /* load */
    OpST,                 /* store */
    OpJSR = 0100 << 12,   /* jump register */
    OpAND = 0101 << 12,   /* bitwise and */
    OpLDR,                /* load register */
    OpSTR,                /* store register */
    OpRTI,                /* unused */
    OpNOT,                /* bitwise not */
    OpLDI,                /* load indirect */
    OpSTI,                /* store indirect */
    OpJMP = 1100 << 12,   /* jump */
    OpRES,                /* reserved (unused) */
    OpLEA,                /* load effective address */
    OpTRAP,               /* execute trap */
}

enum Flag {
    FlPos,  /* Set when the result of the previous operation was positive */
    FlZero, /* Set when the result of the previous operation was zero */
    FlNeg,  /* Set when the result of the previous operation was negative */
    FlNA /* Used for type conversion purposes */
}

//I implement indexing arrays with flag values to make code more declarative
impl<T> Index<Flag> for [T] {
    type Output = T;
    fn index(&self, idx: Flag) -> &Self::Output {
        match idx {
            FlPos => &self[0],
            FlZero => &self[1],
            FlNeg => &self[2],
            FlNA => &self[3]
        }
    }
}

impl<T> IndexMut<Flag> for [T] {
    fn index_mut(&mut self, idx: Flag) -> &mut Self::Output {
        match idx {
            FlPos => &mut self[0],
            FlZero => &mut self[1],
            FlNeg => &mut self[2],
            FlNA => &mut self[3]
        }
    }
}

impl From<u16> for Flag {
    fn from(item: u16) -> Self {
        match item {
            0b1 => FlPos,
            0b10 => FlZero,
            0b100 => FlNeg,
            _ => FlNA
        }
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
    println!("Hello, world!");
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

        vm.add(add_instruction);

        assert_eq!(vm.general_registers[R2], 9);
    }

    #[test]
    fn add_writes_value_in_register_correctly_for_register_and_immediate() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 5;

        let add_instruction =
            (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 4;

        vm.add(add_instruction);

        assert_eq!(vm.general_registers[R2], 9);
    }

    #[test]
    fn add_does_subtraction_correctly_for_immediate() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 5;

        let add_instruction =
            (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 011011; // 0b11111011 is -5 in two's complement

        vm.add(add_instruction);

        assert_eq!(vm.general_registers[R2] as i16, 0);
    }

    #[test]
    fn add_does_subtraction_correctly_for_second_register() {
        let mut vm = LC3VM::new();

        vm.general_registers[R0] = 5;
        vm.general_registers[R1] = 0b1111111111111011; //-6 in two's complement

        let add_instruction =
            (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | R0 as u16;

        vm.add(add_instruction);

        assert_eq!(vm.general_registers[R2] as i16, 0);
    }

    #[test]
    fn add_updates_neg_flag_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 5;

        let add_instruction =
            (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 0b11010; // 0b11010 is -6 in two's complement with 5 bits

        vm.add(add_instruction);

        assert_eq!(vm.condition_flags[FlNeg], 1);
    }

    #[test]
    fn add_updates_zero_flag_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 5;

        let add_instruction =
            (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 0b11011; // 0b11011 is -5 in two's complement for a 5 bit value

        vm.add(add_instruction);

        assert_eq!(vm.condition_flags[FlZero], 1);
    }

    #[test]
    fn add_updates_pos_flag_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 5;

        let add_instruction =
            (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 0b1;

        vm.add(add_instruction);

        assert_eq!(vm.condition_flags[FlPos], 1);
    }

    #[test]
    fn and_writes_value_in_register_correctly_for_register_and_immediate() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 0b101;

        let and_instruction =
            (OpCode::OpAND as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 0b01001;

        vm.and(and_instruction);

        assert_eq!(vm.general_registers[R2], 0b00001);
    }

    #[test]
    fn and_writes_value_in_register_correctly_for_two_registers() {
        let mut vm = LC3VM::new();

        vm.general_registers[R0] = 0b1001;
        vm.general_registers[R1] = 0b101;

        let and_instruction =
            (OpCode::OpAND as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | R0 as u16;

        vm.and(and_instruction);

        assert_eq!(vm.general_registers[R2], 0b00001);
    }

    #[test]
    fn and_updates_neg_flag_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 0xF000;

        let and_instruction =
            (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 0b11010; // 0b11111010 is -6 in two's complement

        vm.and(and_instruction);

        assert_eq!(vm.condition_flags[FlNeg], 1);
    }

    #[test]
    fn and_updates_zero_flag_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 0;

        let and_instruction =
            (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 0b00110; // 0b11011 is -5 in two's complement for a 5 bit value

        vm.and(and_instruction);

        assert_eq!(vm.condition_flags[FlZero], 1);
    }

    #[test]
    fn and_updates_pos_flag_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 5;

        let and_instruction =
            (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 0b1;

        vm.and(and_instruction);

        assert_eq!(vm.condition_flags[FlPos], 1);
    }

    #[test]
    fn not_writes_value_in_register_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R0] = 0xFFF0 | 0b1001;

        let not_instruction = (OpCode::OpAND as u16) | ((R1 as u16) << 9) | ((R0 as u16) << 6);

        vm.not(not_instruction);

        assert_eq!(vm.general_registers[R1], (0x0000 | 0b0110));
    }

    #[test]
    fn not_updates_neg_flag_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R0] = 0;

        let not_instruction = (OpCode::OpADD as u16) | ((R1 as u16) << 9) | ((R0 as u16) << 6);

        vm.not(not_instruction);

        assert_eq!(vm.condition_flags[FlNeg], 1);
    }

    #[test]
    fn not_updates_zero_flag_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R0] = 0xFFFF;

        let not_instruction = (OpCode::OpADD as u16) | ((R1 as u16) << 9) | ((R0 as u16) << 6);

        vm.not(not_instruction);

        assert_eq!(vm.condition_flags[FlZero], 1);
    }

    #[test]
    fn not_updates_pos_flag_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R0] = 0xF000;

        let not_instruction = (OpCode::OpADD as u16) | ((R1 as u16) << 9) | ((R0 as u16) << 6);

        vm.not(not_instruction);

        assert_eq!(vm.condition_flags[FlPos], 1);
    }
    
    #[test]
    fn branch_instruction_branches_for_neg() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 5;

        let add_instruction =
            (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 0b11010; // 0b11010 is -6 in two's complement with 5 bits

        vm.add(add_instruction);

        let branch_instruction = (OpCode::OpBR as u16) | (0b100 << 9) | 2;

        vm.branch(branch_instruction);

        assert_eq!(vm.program_counter, 2);
    }

    #[test]
    fn branch_instruction_branches_for_zero() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 6;

        let add_instruction =
            (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 0b11010; // 0b11010 is -6 in two's complement with 5 bits

        vm.add(add_instruction);

        let branch_instruction = (OpCode::OpBR as u16) | (0b010 << 9) | 2;

        vm.branch(branch_instruction);

        assert_eq!(vm.program_counter, 2);
    }

    #[test]
    fn branch_instruction_branches_for_positive() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 7;

        let add_instruction =
            (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 0b11010; // 0b11010 is -6 in two's complement with 5 bits

        vm.add(add_instruction);

        let branch_instruction = (OpCode::OpBR as u16) | (0b001 << 9) | 2;

        vm.branch(branch_instruction);

        assert_eq!(vm.program_counter, 2);
    }

    #[test]
    fn branch_instruction_doesnt_branch_with_no_flags() {
        let mut vm = LC3VM::new();

        let branch_instruction = (OpCode::OpBR as u16) | (0b001 << 9) | 2;

        vm.branch(branch_instruction);

        assert_eq!(vm.program_counter, 0);
    }

    #[test]
    fn jump_instruction_works_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R2] = 2;

        let jump_instruction = (OpCode::OpJMP as u16) | (0b000 << 9) | ((R2 as u16) << 6);

        vm.jump(jump_instruction);

        assert_eq!(vm.program_counter, 2);
    }
}
