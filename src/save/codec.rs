//! Little-endian primitives for save sections: a `Writer` that appends and a
//! `Reader` that fails (never panics, never over-allocates) on truncated or
//! corrupt data.

use esp::FormId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Corrupt(pub String);

impl std::fmt::Display for Corrupt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "corrupt save: {}", self.0)
    }
}

impl std::error::Error for Corrupt {}

pub type Result<T> = std::result::Result<T, Corrupt>;

#[derive(Default)]
pub struct Writer(pub Vec<u8>);

impl Writer {
    pub fn u8(&mut self, v: u8) {
        self.0.push(v);
    }

    pub fn bool(&mut self, v: bool) {
        self.u8(v as u8);
    }

    pub fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }

    pub fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }

    pub fn i32(&mut self, v: i32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }

    pub fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }

    pub fn f32(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }

    pub fn f64(&mut self, v: f64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }

    pub fn form(&mut self, v: FormId) {
        self.u32(v.0);
    }

    pub fn str(&mut self, v: &str) {
        self.u32(v.len() as u32);
        self.0.extend_from_slice(v.as_bytes());
    }

    /// A count or length prefix.
    pub fn len(&mut self, n: usize) {
        self.u32(n as u32);
    }

    pub fn opt_form(&mut self, v: Option<FormId>) {
        self.form(v.unwrap_or(FormId::NULL));
    }

    pub fn vec3(&mut self, v: glam::Vec3) {
        for c in v.to_array() {
            self.f32(c);
        }
    }
}

pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Reader { data, pos: 0 }
    }

    pub fn left(&self) -> usize {
        self.data.len() - self.pos
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if n > self.left() {
            return Err(Corrupt(format!(
                "wanted {n} bytes at {}, {} left",
                self.pos,
                self.left()
            )));
        }
        let out = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(out)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        Ok(self.take(N)?.try_into().unwrap())
    }

    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    pub fn bool(&mut self) -> Result<bool> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            n => Err(Corrupt(format!("{n} is not a boolean"))),
        }
    }

    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.array()?))
    }

    pub fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    pub fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_le_bytes(self.array()?))
    }

    pub fn f64(&mut self) -> Result<f64> {
        Ok(f64::from_le_bytes(self.array()?))
    }

    pub fn form(&mut self) -> Result<FormId> {
        Ok(FormId(self.u32()?))
    }

    pub fn opt_form(&mut self) -> Result<Option<FormId>> {
        Ok(Some(self.form()?).filter(|f| !f.is_null()))
    }

    pub fn vec3(&mut self) -> Result<glam::Vec3> {
        Ok(glam::Vec3::new(self.f32()?, self.f32()?, self.f32()?))
    }

    pub fn str(&mut self) -> Result<String> {
        let n = self.u32()? as usize;
        let bytes = self.take(n)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| Corrupt("text isn't UTF-8".into()))
    }

    /// A count of items that each take at least `each` bytes: refuses counts
    /// the rest of the data can't hold, so a damaged length can't allocate
    /// gigabytes.
    pub fn count(&mut self, each: usize) -> Result<usize> {
        let n = self.u32()? as usize;
        if n.saturating_mul(each.max(1)) > self.left() {
            return Err(Corrupt(format!(
                "{n} entries of {each} bytes in {} bytes",
                self.left()
            )));
        }
        Ok(n)
    }

    /// That all of a section was read.
    pub fn finish(&self) -> Result<()> {
        match self.left() {
            0 => Ok(()),
            n => Err(Corrupt(format!("{n} unread bytes"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitives_round_trip() {
        let mut w = Writer::default();
        w.u8(7);
        w.bool(true);
        w.u16(513);
        w.u32(0xdead_beef);
        w.i32(-5);
        w.u64(1 << 40);
        w.f32(1.5);
        w.f64(-2.25);
        w.form(FormId(0x14));
        w.opt_form(None);
        w.str("Whiterun");
        w.vec3(glam::Vec3::new(1.0, 2.0, 3.0));
        let mut r = Reader::new(&w.0);
        assert_eq!(r.u8().unwrap(), 7);
        assert!(r.bool().unwrap());
        assert_eq!(r.u16().unwrap(), 513);
        assert_eq!(r.u32().unwrap(), 0xdead_beef);
        assert_eq!(r.i32().unwrap(), -5);
        assert_eq!(r.u64().unwrap(), 1 << 40);
        assert_eq!(r.f32().unwrap(), 1.5);
        assert_eq!(r.f64().unwrap(), -2.25);
        assert_eq!(r.form().unwrap(), FormId(0x14));
        assert_eq!(r.opt_form().unwrap(), None);
        assert_eq!(r.str().unwrap(), "Whiterun");
        assert_eq!(r.vec3().unwrap(), glam::Vec3::new(1.0, 2.0, 3.0));
        r.finish().unwrap();
    }

    #[test]
    fn truncated_and_inflated_data_fail_cleanly() {
        let mut r = Reader::new(&[1, 2]);
        assert!(r.u32().is_err());
        assert!(Reader::new(&[2]).bool().is_err());
        // A string longer than what's left.
        let mut w = Writer::default();
        w.u32(1_000_000);
        assert!(Reader::new(&w.0).str().is_err());
        // A count the rest can't hold.
        let mut w = Writer::default();
        w.u32(u32::MAX);
        w.u32(0);
        assert!(Reader::new(&w.0).count(8).is_err());
        assert!(Reader::new(&[0, 0, 0, 0, 9]).finish().is_err());
    }
}
