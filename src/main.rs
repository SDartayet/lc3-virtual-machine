use std::{ops::{Index, IndexMut, Shl}, slice::SliceIndex};

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
    R7 = 7
}

struct LC3VM {
    general_registers: [u16;8],
    program_counter: u16,
    condition_flags: [u16;3],
    memory: [u16;MAX_MEMORY_ADDRESS as usize]
}

impl LC3VM {

    fn new() -> LC3VM {
        LC3VM { general_registers: [0;8], program_counter: 0, condition_flags: [0;3], memory: [0;MAX_MEMORY_ADDRESS as usize] }
    }

    /// Updates the condition flags according to the result of the last arithmetic operations
    /// Input: the register on which the result was stored
    fn update_flags(&mut self, register_number: usize) {
        let register_value = self.general_registers[register_number];
        if register_value == 0 {
            self.condition_flags[Flag::FlZero] = 1;
        } else if register_value & 0b1000000000000000 != 0 { //In two's complement, if the first bit is one, the number is negative
            self.condition_flags[Flag::FlNeg] = 1;
        } else {
            self.condition_flags[Flag::FlPos] = 1;
        }
    }

    /// Implements the addition instruciton
    /// Instruction structure: OPCODE (4 bits) | Destination register (3 bits) | Source register 1 (3 bits) | Mode (immediate or another register, 1 bit) | [00 | Source register 2 (3 bits)] on mode 0 or a 5 bit value on mode 1
    fn add(&mut self, instruction: lc3_instruction) {
        //I "push" the bits for the register number to the rightmost position, and make all the other bits 0 by doing a bitwise AND with 0b111
        let source_register_1_number= ((instruction >> 6) & 0b111) as usize; 

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
            // If the first bit of the immediate value is negative, because of how two's complement works, we need to extend it with ones unti lwe have 16 bits to preserve the sign
            // I check if the first bit of the 5 bit immediate is one, and if it is I extend it with ones, otherwise with zeroes
            let five_bit_immediate = instruction & 0b11111; // I filter the first 5 bits of the instruction, which contain the immediate, and set the rest to zero
            let is_immediate_negative = (five_bit_immediate & 0b10000) != 0;
            let sign_extension: u16;

            // If the sign extension is negative, I fill with 11 bits of 1, since 11+5 = 16. Otherwise I fill with zeroes
            if is_immediate_negative { sign_extension = 0xFFE0; } else { sign_extension = 0x0000; }

            // Doing a bitwise or between the sign extension I need and the immediate value I get the immediate with the sign extended
            second_operand = sign_extension | five_bit_immediate;
        }

        //Wrapping add lets us recreate the way addition works in two's complement systems while keeping the values unsigned (for more generalization)
        self.general_registers[destination_register_number] = self.general_registers[source_register_1_number].wrapping_add(second_operand);

        self.update_flags(destination_register_number);
    }

    /// Implements the bitwise NOT instruction
    /// Instruction structure: OPCODE (4 bits) | Destination register (3 bits) | Source register (3 bits) | 111111
    fn not(&mut self, instruction: lc3_instruction) {
        //I "push" the bits for the register number to the rightmost position, and make all the other bits 0 by doing a bitwise AND with 0b111
        let source_register_number= ((instruction >> 6) & 0b111) as usize; 

        //Same as previously, but I need to shift right by 9 bits given there are 9 bits befrore the destination register
        let destination_register_number = ((instruction >> 9) & 0b111) as usize; 
        
        self.general_registers[destination_register_number] = !self.general_registers[source_register_number];

        self.update_flags(destination_register_number);
    }

    /// Implements the bitwise AND instruction
    /// Instruction structure: OPCODE (4 bits) | Destination register (3 bits) | Source register 1 (3 bits) | Mode (immediate or another register, 1 bit) | [00 Source register 2 (3 bits)] on mode 0 or a 5 bit value on mode 1
    fn and(&mut self, instruction: lc3_instruction) {
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
       self.general_registers[destination_register_number] = self.general_registers[source_register_1_number] & second_operand;

       self.update_flags(destination_register_number);
    }
}

/// The opcodes for the instructions the architecture supports
enum OpCode {
    OpBR,   /* branch */
    OpADD = 0b0001 << 12,  /* add  */
    OpLD,   /* load */
    OpST,   /* store */
    OpJSR,  /* jump register */
    OpAND = 0101 << 12,  /* bitwise and */
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

//I implement indexing arrays with flag values to make code more declarative
impl<T> Index<Flag> for [T] {
    type Output = T;
    fn index(&self, idx: Flag) -> &Self::Output {
        match idx {
            Flag::FlPos => &self[0],
            Flag::FlZero => &self[1],
            Flag::FlNeg => &self[2],
        }
    }
}

impl<T> IndexMut<Flag> for [T] {
    fn index_mut(&mut self, idx: Flag) -> &mut Self::Output {
        match idx {
            Flag::FlPos => &mut self[0],
            Flag::FlZero => &mut self[1],
            Flag::FlNeg => &mut self[2],
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
            GeneralPurposeRegister::R0 => & mut self[0],
            GeneralPurposeRegister::R1 => & mut self[1],
            GeneralPurposeRegister::R2 => & mut self[2],
            GeneralPurposeRegister::R3 => & mut self[3],
            GeneralPurposeRegister::R4 => & mut self[4],
            GeneralPurposeRegister::R5 => & mut self[5],
            GeneralPurposeRegister::R6 => & mut self[6],
            GeneralPurposeRegister::R7 => & mut self[7],
        }
    }
}

fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
mod tests{
    use super::*;
    use super::GeneralPurposeRegister::{*};
    use super::Flag::{*};

    #[test]
    fn add_writes_value_in_register_correctly_for_2_register_add() {
        let mut vm = LC3VM::new();

        vm.general_registers[R0] = 5;
        vm.general_registers[R1] = 4;

        let add_instruction = (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | R0 as u16;

        vm.add(add_instruction);

        assert_eq!(vm.general_registers[R2], 9);
    }

    #[test]
    fn add_writes_value_in_register_correctly_for_register_and_immediate() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 5;

        let add_instruction = (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 4;

        vm.add(add_instruction);

        assert_eq!(vm.general_registers[R2], 9);
    }

    #[test]
    fn add_does_subtraction_correctly_for_immediate() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 5;

        let add_instruction = (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 011011; // 0b11111011 is -5 in two's complement

        vm.add(add_instruction);

        assert_eq!(vm.general_registers[R2] as i16,0);
    }

    #[test]
    fn add_does_subtraction_correctly_for_second_register() {
        let mut vm = LC3VM::new();

        vm.general_registers[R0] = 5;
        vm.general_registers[R1] = 0b1111111111111011; //-6 in two's complement

        let add_instruction = (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | R0 as u16;

        vm.add(add_instruction);

        assert_eq!(vm.general_registers[R2] as i16,0);
    }

    #[test]
    fn add_updates_neg_flag_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 5;

        let add_instruction = (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 0b11010; // 0b11010 is -6 in two's complement with 5 bits

        vm.add(add_instruction);

        assert_eq!(vm.condition_flags[FlNeg],1);
    }

    #[test]
    fn add_updates_zero_flag_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 5;

        let add_instruction = (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 0b11011; // 0b11011 is -5 in two's complement for a 5 bit value

        vm.add(add_instruction);

        assert_eq!(vm.condition_flags[FlZero],1);
    }

    #[test]
    fn add_updates_pos_flag_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 5;

        let add_instruction = (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 0b1; 

        vm.add(add_instruction);

        assert_eq!(vm.condition_flags[FlPos],1);
    }


    #[test]
    fn and_writes_value_in_register_correctly_for_register_and_immediate() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 0b101;

        let and_instruction = (OpCode::OpAND as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 0b01001;

        vm.and(and_instruction);

        assert_eq!(vm.general_registers[R2], 0b00001);
    }

    #[test]
    fn and_writes_value_in_register_correctly_for_two_registers() {
        let mut vm = LC3VM::new();

        vm.general_registers[R0] = 0b1001;
        vm.general_registers[R1] = 0b101;

        let and_instruction = (OpCode::OpAND as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | R0 as u16;

        vm.and(and_instruction);

        assert_eq!(vm.general_registers[R2], 0b00001);
    }

    #[test]
    fn and_updates_neg_flag_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 0xF000;

        let and_instruction = (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 0b11010; // 0b11111010 is -6 in two's complement

        vm.and(and_instruction);

        assert_eq!(vm.condition_flags[FlNeg],1);
    }

    #[test]
    fn and_updates_zero_flag_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 0;

        let and_instruction = (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 0b00110; // 0b11011 is -5 in two's complement for a 5 bit value

        vm.and(and_instruction);

        assert_eq!(vm.condition_flags[FlZero],1);
    }

    #[test]
    fn and_updates_pos_flag_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R1] = 5;

        let and_instruction = (OpCode::OpADD as u16) | ((R2 as u16) << 9) | ((R1 as u16) << 6) | (1 << 5) | 0b1; 

        vm.and(and_instruction);

        assert_eq!(vm.condition_flags[FlPos],1);
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

        assert_eq!(vm.condition_flags[FlNeg],1);
    }

    #[test]
    fn not_updates_zero_flag_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R0] = 0xFFFF;

        let not_instruction = (OpCode::OpADD as u16) | ((R1 as u16) << 9) | ((R0 as u16) << 6); 

        vm.not(not_instruction);

        assert_eq!(vm.condition_flags[FlZero],1);
    }

    #[test]
    fn not_updates_pos_flag_correctly() {
        let mut vm = LC3VM::new();

        vm.general_registers[R0] = 0xF000;

        let not_instruction = (OpCode::OpADD as u16) | ((R1 as u16) << 9) | ((R0 as u16) << 6); 

        vm.not(not_instruction);

        assert_eq!(vm.condition_flags[FlPos],1);
    }
}
