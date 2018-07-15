extern crate rand;
extern crate byteorder;

mod endpoint;
mod mod_ord;
mod helper;

pub use helper::{
	Guarantee,
	EndpointConfig,
	UdpLike,
	Sender,
};

pub use endpoint::{
	Endpoint,
	SetSender,
};

////////////////////////////////////////////////////////////////
#[cfg(test)]
extern crate serde;

#[cfg(test)]
#[macro_use]
extern crate serde_derive;

#[cfg(test)]
extern crate bincode;

#[cfg(test)]
extern crate mio;

#[cfg(test)]
mod tests;