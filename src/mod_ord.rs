
use byteorder::{ReadBytesExt, WriteBytesExt, BigEndian};
use std::io;
use std::cmp::{
	Ordering,
	// Ord,
	// PartialOrd,
};
use std;

const HALF: u32 = std::u32::MAX/2;
const MAX: u32 = std::u32::MAX;

#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug)]
pub struct ModOrd(u32);
impl ModOrd {
	#[inline(always)]
	pub fn new() -> Self { ModOrd(0) }
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
    	ModOrd(self.0 + num)
    }
    pub fn read_from<R: io::Read>(mut r:R) -> Result<ModOrd, io::Error> {
    	r.read_u32::<BigEndian>().map(|x| ModOrd(x))
    }
    pub fn write_to<W: io::Write>(self, mut w:W) -> Result<(), io::Error> {
    	w.write_u32::<BigEndian>(self.0)
    }
}