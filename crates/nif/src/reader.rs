use glam::{Mat3, Vec2, Vec3, Vec4};

use crate::{Error, Result};

pub struct Reader<'a> {
    pub data: &'a [u8],
    pub pos: usize,
    pub bs_version: u32,
    pub strings: &'a [String],
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Reader {
            data,
            pos: 0,
            bs_version: 0,
            strings: &[],
        }
    }
    pub fn pos(&self) -> usize {
        self.pos
    }
    pub fn set_pos(&mut self, p: usize) {
        self.pos = p;
    }
    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }
    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.pos + n > self.data.len() {
            return Err(Error::Eof(self.pos));
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    pub fn skip(&mut self, n: usize) -> Result<()> {
        self.bytes(n).map(|_| ())
    }
    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.bytes(1)?[0])
    }
    pub fn bool(&mut self) -> Result<bool> {
        Ok(self.u8()? != 0)
    }
    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.bytes(2)?.try_into().unwrap()))
    }
    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }
    pub fn i32(&mut self) -> Result<i32> {
        Ok(self.u32()? as i32)
    }
    pub fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.bytes(8)?.try_into().unwrap()))
    }
    pub fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_bits(self.u32()?))
    }
    pub fn vec2(&mut self) -> Result<Vec2> {
        Ok(Vec2::new(self.f32()?, self.f32()?))
    }
    pub fn vec3(&mut self) -> Result<Vec3> {
        Ok(Vec3::new(self.f32()?, self.f32()?, self.f32()?))
    }
    pub fn vec4(&mut self) -> Result<Vec4> {
        Ok(Vec4::new(
            self.f32()?,
            self.f32()?,
            self.f32()?,
            self.f32()?,
        ))
    }
    /// Gamebryo matrices are row-major and transform column vectors.
    pub fn mat3(&mut self) -> Result<Mat3> {
        let mut a = [0f32; 9];
        for v in &mut a {
            *v = self.f32()?;
        }
        Ok(Mat3::from_cols_array(&a).transpose())
    }
    pub fn line(&mut self) -> Result<String> {
        let start = self.pos;
        while self.pos < self.data.len() && self.data[self.pos] != b'\n' {
            self.pos += 1;
        }
        if self.pos >= self.data.len() {
            return Err(Error::Eof(self.pos));
        }
        let s = String::from_utf8_lossy(&self.data[start..self.pos]).into_owned();
        self.pos += 1;
        Ok(s)
    }
    /// u8 length-prefixed string (null terminated in practice).
    pub fn short_string(&mut self) -> Result<String> {
        let n = self.u8()? as usize;
        let b = self.bytes(n)?;
        let b = b.strip_suffix(&[0]).unwrap_or(b);
        Ok(latin1(b))
    }
    /// u32 length-prefixed string.
    pub fn sized_string(&mut self) -> Result<String> {
        let n = self.u32()? as usize;
        if n > self.remaining() {
            return Err(Error::Invalid(format!("string length {n} too large")));
        }
        Ok(latin1(self.bytes(n)?))
    }
    pub fn string_ref(&mut self) -> Result<crate::StringRef> {
        let i = self.u32()?;
        Ok(crate::StringRef(if i == u32::MAX { None } else { Some(i) }))
    }
    pub fn string_value(&mut self) -> Result<String> {
        let s = self.string_ref()?;
        Ok(s.0
            .and_then(|i| self.strings.get(i as usize))
            .cloned()
            .unwrap_or_default())
    }
    pub fn block_ref(&mut self) -> Result<crate::Ref> {
        Ok(crate::Ref(self.i32()?))
    }
    pub fn ref_list(&mut self) -> Result<Vec<crate::Ref>> {
        let n = self.u32()? as usize;
        if n * 4 > self.remaining() {
            return Err(Error::Invalid("ref list too long".into()));
        }
        (0..n).map(|_| self.block_ref()).collect()
    }
}

pub fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| c as char).collect()
}

pub fn half_to_f32(h: u16) -> f32 {
    let sign = ((h >> 15) & 1) as u32;
    let exp = ((h >> 10) & 0x1F) as u32;
    let mant = (h & 0x3FF) as u32;
    let bits = if exp == 0 {
        if mant == 0 {
            sign << 31
        } else {
            // subnormal
            let mut e = 127 - 15 + 1;
            let mut m = mant;
            while m & 0x400 == 0 {
                m <<= 1;
                e -= 1;
            }
            (sign << 31) | ((e as u32) << 23) | ((m & 0x3FF) << 13)
        }
    } else if exp == 31 {
        (sign << 31) | (0xFF << 23) | (mant << 13)
    } else {
        (sign << 31) | ((exp + 127 - 15) << 23) | (mant << 13)
    };
    f32::from_bits(bits)
}
