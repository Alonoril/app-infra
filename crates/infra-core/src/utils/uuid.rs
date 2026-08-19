use std::fmt::{self, Display, Formatter};
use uuid::{Uuid, fmt::Simple};

const HEX_LOWER: &[u8; 16] = b"0123456789abcdef";

/// A UUID v4 trace identifier that formats as 32 lower-case hexadecimal bytes without allocating.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TraceId(Uuid);

/// An 8-byte trace identifier view that formats as 16 lower-case hexadecimal bytes without allocating.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TraceId8([u8; 8]);

impl TraceId {
	/// Encodes this trace ID into a caller-owned stack buffer.
	#[inline]
	pub fn encode_lower<'buffer>(&self, buffer: &'buffer mut [u8; 32]) -> &'buffer str {
		self.0.simple().encode_lower(buffer)
	}

	/// Returns the first 8 raw UUID bytes as a displayable compact trace ID.
	#[inline]
	pub fn front_8(&self) -> TraceId8 {
		let bytes = self.0.as_bytes();
		let mut front = [0; 8];
		front.copy_from_slice(&bytes[..8]);
		TraceId8(front)
	}

	/// Returns the last 8 raw UUID bytes as a displayable compact trace ID.
	#[inline]
	pub fn back_8(&self) -> TraceId8 {
		let bytes = self.0.as_bytes();
		let mut back = [0; 8];
		back.copy_from_slice(&bytes[8..]);
		TraceId8(back)
	}
}

impl Display for TraceId {
	#[inline]
	fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
		Display::fmt(&self.0.simple(), formatter)
	}
}

impl TraceId8 {
	/// Returns the raw 8 bytes used by this compact trace ID.
	#[inline]
	pub fn as_bytes(&self) -> &[u8; 8] {
		&self.0
	}

	/// Encodes this compact trace ID into a caller-owned stack buffer.
	#[inline]
	pub fn encode_lower<'buffer>(&self, buffer: &'buffer mut [u8; 16]) -> &'buffer str {
		encode_bytes8_lower(&self.0, buffer)
	}
}

impl Display for TraceId8 {
	#[inline]
	fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
		let mut buffer = [0; 16];
		formatter.write_str(self.encode_lower(&mut buffer))
	}
}

pub struct UID;

impl UID {
	#[inline]
	pub fn v4(&self) -> Uuid {
		Uuid::new_v4()
	}

	#[inline]
	pub fn v4_simple(&self) -> Simple {
		self.v4().simple()
	}

	/// Creates a UUID v4-backed trace ID whose formatting path does not allocate.
	#[inline]
	pub fn trace_id(&self) -> TraceId {
		TraceId(self.v4())
	}

	/// Creates a UUID v4-backed 8-byte trace ID whose formatting path does not allocate.
	///
	/// This uses the last 8 UUID bytes to avoid the UUID version and variant bits in the first half.
	#[inline]
	pub fn trace_id8(&self) -> TraceId8 {
		self.trace_id().back_8()
	}

	/// Creates a compact random ID from the low 16 bits of a UUID v4.
	///
	/// This has a high collision probability and is suitable only when uniqueness is not required.
	#[inline]
	pub fn v4_u16(&self) -> u16 {
		self.v4().as_u128() as u16
	}

	/// Creates a compact random ID from the low 32 bits of a UUID v4.
	///
	/// This can collide and must not be used where global uniqueness is required.
	#[inline]
	pub fn v4_u32(&self) -> u32 {
		self.v4().as_u128() as u32
	}

	#[inline]
	pub fn v4_low_u64(&self) -> u64 {
		let (_, low) = self.v4().as_u64_pair();
		low
	}
}

#[inline]
fn encode_bytes8_lower<'buffer>(bytes: &[u8; 8], buffer: &'buffer mut [u8; 16]) -> &'buffer str {
	for (index, byte) in bytes.iter().copied().enumerate() {
		let offset = index * 2;
		buffer[offset] = HEX_LOWER[(byte >> 4) as usize];
		buffer[offset + 1] = HEX_LOWER[(byte & 0x0f) as usize];
	}

	// The lookup table contains only ASCII hex characters, so this buffer is always valid UTF-8.
	unsafe { str::from_utf8_unchecked(buffer) }
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::fmt::{self, Write};

	struct StackBuffer {
		bytes: [u8; 32],
		len: usize,
	}

	impl StackBuffer {
		fn new() -> Self {
			Self { bytes: [0; 32], len: 0 }
		}

		fn as_str(&self) -> &str {
			str::from_utf8(&self.bytes[..self.len]).unwrap()
		}
	}

	impl Write for StackBuffer {
		fn write_str(&mut self, value: &str) -> fmt::Result {
			let end = self.len + value.len();
			self.bytes[self.len..end].copy_from_slice(value.as_bytes());
			self.len = end;
			Ok(())
		}
	}

	#[test]
	fn trace_id_formats_as_simple_uuid_without_heap_backed_output() {
		let trace_id = UID.trace_id();
		let mut output = StackBuffer::new();

		write!(&mut output, "{trace_id}").unwrap();

		assert_eq!(output.as_str().len(), 32);
		assert!(
			output
				.as_str()
				.bytes()
				.all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
		);
	}

	#[test]
	fn trace_id_encodes_into_caller_buffer() {
		let trace_id = UID.trace_id();
		println!("trace_id: {}", trace_id);
		let mut buffer = [0; 32];

		let encoded = trace_id.encode_lower(&mut buffer);

		assert_eq!(encoded.len(), 32);
		assert_eq!(encoded, trace_id.to_string());
	}

	#[test]
	fn trace_id_front_8_formats_first_8_bytes_without_heap_backed_output() {
		let trace_id = TraceId(Uuid::from_bytes([
			0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff,
		]));
		let front = trace_id.front_8();
		let mut output = StackBuffer::new();

		write!(&mut output, "{front}").unwrap();

		assert_eq!(front.as_bytes(), &[0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77]);
		assert_eq!(output.as_str(), "0011223344556677");
	}

	#[test]
	fn trace_id_back_8_encodes_last_8_bytes_into_caller_buffer() {
		let trace_id = TraceId(Uuid::from_bytes([
			0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff,
		]));
		let back = trace_id.back_8();
		let mut buffer = [0; 16];

		let encoded = back.encode_lower(&mut buffer);

		assert_eq!(back.as_bytes(), &[0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
		assert_eq!(encoded, "8899aabbccddeeff");
	}

	#[test]
	fn trace_id8_formats_as_16_lower_hex_bytes() {
		let trace_id = UID.trace_id8();
		let mut output = StackBuffer::new();

		write!(&mut output, "{trace_id}").unwrap();

		assert_eq!(output.as_str().len(), 16);
		assert!(
			output
				.as_str()
				.bytes()
				.all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
		);
	}

	#[test]
	fn creates_compact_numeric_ids() {
		let v16: u16 = UID.v4_u16();
		println!("v16: {}", v16);
		let v32: u32 = UID.v4_u32();
		println!("v32: {}", v32);
	}

	#[test]
	fn test_v4() {
		let my_uuid = UID.v4();
		println!("{:?}", my_uuid);
		// Convert UUID into two u64 values
		let (high, low) = my_uuid.as_u64_pair();

		// Print the result
		println!("High u64: {}", high);
		println!("Low u64: {}", low);
	}

	#[test]
	fn test_v4_simple() {
		let my_uuid = UID.v4_simple();
		println!("{:?}", my_uuid);
	}

	#[test]
	fn test_v4_low_u64() {
		let my_uuid = UID.v4_low_u64();
		println!("{}", my_uuid);
	}
}
