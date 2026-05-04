
use bitflags::bitflags;

bitflags! {
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
    pub struct PermissionedOp: u64 {
        const DEPOSIT =    1 << 0;
        const BORROW =     1 << 1;
        const LIQUIDATE =  1 << 2;
       

        const NONE = 0;
    }
}

impl PermissionedOp {
    #[cfg(feature = "serde")]
    pub fn from_string(str_op: &str) -> Result<u64, bitflags::parser::ParseError> {
        match bitflags::parser::from_str_strict::<PermissionedOp>(str_op) {
            Ok(op) => Ok(op.bits()),
            Err(e) => Err(e),
        }
    }


    pub fn get_all_ops() -> Vec<PermissionedOp> {
        Self::all().into_iter().collect()
    }

    pub fn split(&self) -> Vec<PermissionedOp> {
        (*self).into_iter().collect()
    }

    pub const PERMISSION_ALL: u64 = Self::all().bits();

    pub fn add(&mut self, op: PermissionedOp) {
        self.insert(op);
    }

    pub fn revoke(&mut self, op: PermissionedOp) {
        self.remove(op);
    }
}

impl ToString for PermissionedOp {
    fn to_string(&self) -> String {
        let mut buffer = String::new();
        bitflags::parser::to_writer(self, &mut buffer).expect("Failed to serialize PermissionedOp");
        buffer
    }
}


#[cfg(feature = "serde")]
pub mod bitflags_str {

    use bitflags;

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let input: String = serde::Deserialize::deserialize(deserializer)?;
        super::PermissionedOp::from_string(&input).map_err(serde::de::Error::custom)
    }


    pub fn serialize<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let flags = super::PermissionedOp::from_bits_truncate(*value);

       
        let mut buffer = String::new();
        bitflags::parser::to_writer_strict(&flags, &mut buffer)
            .expect("Failed to serialize PermissionedOp - this should never happen");

        serializer.serialize_str(&buffer)
    }
}
