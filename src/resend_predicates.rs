use std::time::Duration;

pub fn medium_combination(id_difference: u32, age: Duration) -> bool {
	if age.as_secs() > 0 {
		true
	} else {
		let x = age.subsec_millis() + id_difference * 8;
		x >= 1000 
	}
}

pub fn hundred_millis(_id_difference: u32, age: Duration) -> bool {
	age.as_secs() > 0
	|| age.subsec_millis() >= 200
}

pub fn twenty_ids(id_difference: u32, _age: Duration) -> bool {
	id_difference >= 20
}
