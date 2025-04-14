/// Representation for the different errors that can arise in VM execution
#[derive(Copy, Clone, Debug)]
pub enum VMError {
    /// Invalid opcode in the instruction encoding. Stores the instruction
    InvalidOpCode(u8),
    /// Invalid trap code in the instruction encoding. Stores the instruction
    InvalidTrapCode(u8),
    /// Error getting the terminal attributes in setup
    TerminalIOAttributesGet,
    /// Error setting the terminal attributes in setup
    TerminalIOAttributesSet,
    /// Error reading from stdin or writing to stdout
    IOError,
}
