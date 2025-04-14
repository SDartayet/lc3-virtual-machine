///The registers the architecture contains.
/// R0 to R7 are the general purpose registers. PC is the instruction pointer.
/// RCOND is the flags register,
#[derive(Clone, Copy)]
pub enum GeneralPurposeRegister {
    R0 = 0,
    R1 = 1,
    R2 = 2,
    R3 = 3,
    R4 = 4,
    R5 = 5,
    R6 = 6,
    R7 = 7,
}

///Code for a trap routine to be executed. The code is its location in memory
pub enum TrapCode {
    /// Get character from keyboard, not echoed onto the terminal
    TrapGETC = 0x20,
    /// Output a character
    TrapOUT = 0x21,
    /// Output a word string
    TrapPUTS = 0x22,
    /// Get character from keyboard, echoed onto the terminal
    TrapIN = 0x23,
    /// Output a byte string
    TrapPUTSP = 0x24,
    /// Halt the program
    TrapHALT = 0x25,
}

/// The opcodes for the instructions the architecture supports
#[derive(Debug, PartialEq, Eq)]
pub enum OpCode {
    /// Branch
    OpBR = 0b0000 << 12,
    /// Add  
    OpADD = 0b0001 << 12,
    /// Load
    OpLD = 0b0010 << 12,
    /// Store
    OpST = 0b0011 << 12,
    /// Jump register
    OpJSR = 0b0100 << 12,
    /// Bitwise and   
    OpAND = 0b0101 << 12,
    /// Load register
    OpLDR = 0b0110 << 12,
    /// Store register
    OpSTR = 0b0111 << 12,
    /// Bitwise not
    OpNOT = 0b1001 << 12,
    /// Load indirect
    OpLDI = 0b1010 << 12,
    /// Store indirect
    OpSTI = 0b1011 << 12,
    /// Jump
    OpJMP = 0b1100 << 12,
    /// Load effective address
    OpLEA = 0b1110 << 12,
    /// Execute trap
    OpTRAP = 0b1111 << 12,
}

/// Values for the different condition flags for comparison with the cond register
pub enum Flag {
    /// Set when the result of the previous operation was positive
    Positive = 0b001,
    /// Set when the result of the previous operation was zero
    Zero = 0b010,
    /// Set when the result of the previous operation was negative
    Negative = 0b100,
}

/// Extends sign for an integer to make it 16 bits
/// Input: an integer in two's complements and the number of bits representing it
/// Output: the integer with its sign extended so it occupies 16 bits
pub fn extend_sign_for_integer(value: u16, value_bit_count: u16) -> u16 {
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
