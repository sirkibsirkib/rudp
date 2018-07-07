use std::collections::VecDeque;

pub struct LilHeap {
	left: usize,
	buf: VecDeque<u8>,
}
impl LilHeap {
	pub fn new() -> Self {
		LilHeap {
			left: usize,
			buf: vec![],
		}
	}

	pub fn write_buf(&mut self) -> &mut [u8] {

	}
}
