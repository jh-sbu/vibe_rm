/// A FormID. Inside a plugin file, the top byte indexes that plugin's master
/// list ("local" ids). Once loaded via [`crate::LoadOrder`], ids are "global":
/// the top byte is the load-order index, or `0xFE` + 12-bit light index for ESL.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct FormId(pub u32);

impl FormId {
    pub const NULL: FormId = FormId(0);
    pub fn is_null(self) -> bool {
        self.0 == 0
    }
}

impl std::fmt::Debug for FormId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:08X}", self.0)
    }
}
impl std::fmt::Display for FormId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:08X}", self.0)
    }
}
