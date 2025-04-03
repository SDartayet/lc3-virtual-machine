//The number of memory addresses is 2^16
const MAX_MEMORY_ADDRESS: u16 = (u32::pow(2, 16) - 1) as u16;
const REGISTER_COUNT: u8 = 10;

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
    FlPos = 1 << 0,  /* Set when the result of the previous operation was positive */
    FlZero = 1 << 1, /* Set when the result of the previous operation was zero */
    FlNeg = 1 << 2,  /* Set when the result of the previous operation was negative */
}

fn main() {
    println!("Hello, world!");
}
