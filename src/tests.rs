// use crossbeam_channel::{
// 	Sender,
// 	Receiver,
// };
// use std::io;
use std::net::UdpSocket;
use super::*;



#[test]
fn go() {
	let loopback = "127.0.0.1";
	let (a1,a2) = (format!("{}:8882", loopback), format!("{}:8883", loopback));
	let a = UdpSocket::bind(&a1).unwrap();
	let b = UdpSocket::bind(&a2).unwrap();

	a.set_nonblocking(true).unwrap();
	b.set_nonblocking(true).unwrap();

	let mut x = Endpoint::new(a, a2.parse().unwrap());
	let mut y = Endpoint::new(b, a1.parse().unwrap());
	let x_buf = (0..8).collect::<Vec<_>>();
	let mut ybuf = [0u8; 128];

	x.sender_do(Guarantee::None, |mut w| w.write(&x_buf[..]).map(|_| ()));

	// x.send(&x_buf, Guarantee::None).unwrap();
	let res = y.try_recv();
	println!("res {:?}", res);
}
