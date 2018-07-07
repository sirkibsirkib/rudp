# Design decisions

writing API
 
1.
send packet with each flush. len taken care of
write is relatively versatile
`fn get_sender(&mut self) -> &mut W: io::Write` and `flush()`

1. 
expose the internal buffer directly. user must specify length with separate call 
+ minimal waste. user can save writes if they like
- ugly and maybe error prone?
`fn get_send_buf(&mut self) -> &mut [u8]; fn send(&mut self, len: u16)`

1.
most clear API. requires a copy over.
+ intuitive and impossible for the user to screw up
- wastes cycles
`fn send(&mut self, payload: &[u8])`
