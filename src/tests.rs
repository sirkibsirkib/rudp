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
	let x_buf = vec![21,21,21,21,21];
	let mut ybuf = [0u8; 128];


	x.write(&x_buf[..]).expect("write bad");
	x.send(Guarantee::Delivery(0)).expect("send bad");
	x.send(Guarantee::None).expect("send bad2");

	println!("\n///////////////////\n");
	for i in 0..5 {

		println!("\nTRY {}:: res {:?}", i,  y.try_recv());
	}
}
