

#[derive(Debug)]
struct SessionState {
	max_yielded: ModOrd, // for acking
	time_last_acked: Instant,
	next_id: ModOrd,
	wait_until: ModOrd,
	peer_acked: ModOrd,
	n: ModOrd,
	largest_set_id_yielded: ModOrd,
	seen_before: HashSet<ModOrd>, // contains messages only in THIS set
}

impl SessionState {
	fn new() -> Self {
		let time_last_acked = Instant::now();
		SessionState {
			next_id: ModOrd::ZERO,
			wait_until: ModOrd::ZERO,
			n: ModOrd::ZERO,
			largest_set_id_yielded: ModOrd::ZERO,
			max_yielded: ModOrd::BEFORE_ZERO,
			peer_acked: ModOrd::BEFORE_ZERO,
			time_last_acked,
			seen_before: HashSet::new(),
		}
	}

	pub fn maintain() {
		  
	}
}