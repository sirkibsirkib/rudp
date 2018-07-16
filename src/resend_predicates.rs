use std::time::Duration;


/// resend iff:
/// age < 1.0 sec
/// OR age_millis + (id_difference*8) >= 1000
pub fn medium_combination(id_difference: u32, age: Duration) -> bool {
	if age.as_secs() > 0 {
		true
	} else {
		let x = age.subsec_millis() + id_difference * 8;
		x >= 1000 
	}
}


/// Resend iff at least 200 milliseconds have elapsed without the message
/// being acknowledged by the peer.
pub fn two_hundred_millis(_id_difference: u32, age: Duration) -> bool {
	age.as_secs() > 0
	|| age.subsec_millis() >= 200
}


/// Resend iff at least twenty newer-id payloads have been sent in the meantime
/// without it being acknowledged by the peer.
pub fn twenty_ids(id_difference: u32, _age: Duration) -> bool {
	id_difference >= 20
}
