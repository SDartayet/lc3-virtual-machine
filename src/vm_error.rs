#[derive(Copy, Clone, Debug)]
pub enum VMError {
    InvalidOpCode(u8),
    InvalidTrapCode(u8),
    TerminalIOAttributesGet,
    TerminalIOAttributesSet,
    IOError,
}
