
pub struct NMethod(*const i8);

impl NMethod {
    pub fn new(inner: *const i8) -> Self {
        Self(inner)
    }


}