use byteorder::{ReadBytesExt, WriteBytesExt, BigEndian};
use std::io;
use mod_ord::ModOrd;

#[derive(Debug)]
pub struct Header {
	pub seq: ModOrd, // 4
	pub seq_minor: u16, // 2
	pub ack: ModOrd, // 4
	pub ack_minor: u16, // 2
	pub seqs_since_del: u16, // 2
	pub seqs_since_del_minor: u16, // 2
}

impl Header {
	pub const LEN: usize = 16;

	pub fn write_to<W: io::Write>(&self, mut w: W) -> Result<(), io::Error> {
		self.seq.write_to(&mut w)?;
		w.write_u16::<BigEndian>(self.seq_minor)?;
		self.ack.write_to(&mut w)?;
		w.write_u16::<BigEndian>(self.ack_minor)?;
		w.write_u16::<BigEndian>(self.seqs_since_del)?;
		w.write_u16::<BigEndian>(self.seqs_since_del_minor)?;
		Ok(())
	}

	pub fn read_from<R: io::Read>(mut r: R) -> Result<Header, io::Error> {
		Ok(Header {
			seq: ModOrd::read_from(&mut r)?,
			seq_minor: r.read_u16::<BigEndian>()?,
			ack: ModOrd::read_from(&mut r)?,
			ack_minor: r.read_u16::<BigEndian>()?,
			seqs_since_del: r.read_u16::<BigEndian>()?,
			seqs_since_del_minor: r.read_u16::<BigEndian>()?,
		})
	}
}