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
pub enum TrapCode {
    TrapGETC = 0x20,  /* get character from keyboard, not echoed onto the terminal */
    TrapOUT = 0x21,   /* output a character */
    TrapPUTS = 0x22,  /* output a word string */
    TrapIN = 0x23,    /* get character from keyboard, echoed onto the terminal */
    TrapPUTSP = 0x24, /* output a byte string */
    TrapHALT = 0x25,  /* halt the program */
}

/// The opcodes for the instructions the architecture supports
#[derive(Debug, PartialEq, Eq)]
pub enum OpCode {
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

pub enum Flag {
    Positive = 0b001, /* Set when the result of the previous operation was positive */
    Zero = 0b010,     /* Set when the result of the previous operation was zero */
    Negative = 0b100, /* Set when the result of the previous operation was negative */
}

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
