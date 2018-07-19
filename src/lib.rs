// extern crate rand;
extern crate byteorder;
#[macro_use]
extern crate derivative;

mod endpoint;
mod mod_ord;
mod helper;


/*
EXPERIMENTAL MODULE. tab out to get things compiling again
*/
mod try2;

/// Provided some example functions that can be passed as the resend_predicate of
/// a customized EndpointConfig. Feel free to use your own functions.
pub mod resend_predicates;

pub use helper::{
	Guarantee,
	EndpointConfig,
	UdpLike,
	Sender,
	NewSetUnsent,
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
extern crate rand;

#[cfg(test)]
mod tests;