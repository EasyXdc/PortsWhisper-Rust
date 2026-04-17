#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExitCode {
    Success,
    Failure,
}

impl ExitCode {
    pub fn as_i32(self) -> i32 {
        match self {
            Self::Success => 0,
            Self::Failure => 1,
        }
    }
}
