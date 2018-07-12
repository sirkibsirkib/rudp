
use byteorder::{ReadBytesExt, WriteBytesExt, BigEndian};
use std::io;
use std::cmp::{
	Ordering,
	// Ord,
	// PartialOrd,
};
use std;

pub const HALF: u32 = std::u32::MAX/2;
pub const MAX: u32 = std::u32::MAX;

#[derive(Copy, Clone, Hash, PartialEq, Eq)]
pub struct ModOrd(u32);
impl ModOrd {
    pub const ZERO: Self = ModOrd(1);
    pub const BEFORE_ZERO: Self = ModOrd(0xFFFF_FFFF);
    pub const SPECIAL: Self = ModOrd(0);

	#[inline(always)]
	pub fn wrap_cmp(&self, other: &Self) -> Ordering {
    	let (s1, s2) = (self.0, other.0);
        if s1 == s2 {
        	Ordering::Equal
        } else if (s1 > s2 && s1 - s2 <= HALF) 
               || (s1 < s2 && s2 - s1  > HALF) {
           	Ordering::Greater
        } else {
        	Ordering::Less
        }
    }
    pub fn new_plus(self, num: u32) -> Self {
    	ModOrd({
            let x = self.0.wrapping_add(num);
            if x < self.0 {
                // it wrapped around!
                x + 1
            } else {
                x
            }
        })
    }
    pub fn read_from<R: io::Read>(mut r:R) -> Result<ModOrd, io::Error> {
    	r.read_u32::<BigEndian>().map(|x| ModOrd(x))
    }
    pub fn write_to<W: io::Write>(self, mut w:W) -> Result<(), io::Error> {
    	w.write_u32::<BigEndian>(self.0)
    }
}

use std::fmt;

impl fmt::Debug for ModOrd {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        if self.0 == 0 {
            write!(f, "(special)")
        } else {
            write!(f, "{}~", self.0)
        }
    }
}