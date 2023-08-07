//! This file implements sockets.

use super::Buffer;
use crate::errno::EResult;
use crate::errno::Errno;
use crate::file::buffer::BlockHandler;
use crate::net::buff::BuffList;
use crate::net::osi;
use crate::net::osi::TransmitPipeline;
use crate::net::SocketDesc;
use crate::net::SocketDomain;
use crate::net::SocketType;
use crate::process::mem_space::MemSpace;
use crate::process::Process;
use crate::syscall::ioctl;
use crate::util::container::ring_buffer::RingBuffer;
use crate::util::container::vec::Vec;
use crate::util::io::IO;
use crate::util::lock::IntMutex;
use crate::util::lock::Mutex;
use crate::util::ptr::arc::Arc;
use crate::util::TryDefault;
use core::cmp::min;
use core::ffi::c_int;
use core::ffi::c_void;

/// The maximum size of a socket's buffers.
const BUFFER_SIZE: usize = 65536;

/// Socket option level: Socket
const SOL_SOCKET: c_int = 1;

/// Structure representing a socket.
pub struct Socket {
	/// The socket's stack descriptor.
	desc: SocketDesc,
	/// The socket's transmit pipeline.
	transmit_pipeline: Option<osi::TransmitPipeline<'static>>,

	/// The buffer containing received data. If `None`, reception has been shutdown.
	receive_buffer: Option<RingBuffer<u8, Vec<u8>>>,
	/// The buffer containing data to be transmitted. If `None`, transmission has been shutdown.
	transmit_buffer: Option<RingBuffer<u8, Vec<u8>>>,

	/// The number of entities owning a reference to the socket. When this count reaches zero, the
	/// socket is closed.
	open_count: u32,

	/// The socket's block handler.
	block_handler: BlockHandler,

	/// The address the socket is bound to.
	sockname: Vec<u8>,
}

impl Socket {
	/// Creates a new instance.
	pub fn new(desc: SocketDesc) -> Result<Arc<Mutex<Self>>, Errno> {
		Arc::new(Mutex::new(Self {
			desc,
			transmit_pipeline: None,

			receive_buffer: Some(RingBuffer::new(crate::vec![0; BUFFER_SIZE]?)),
			transmit_buffer: Some(RingBuffer::new(crate::vec![0; BUFFER_SIZE]?)),

			open_count: 0,

			block_handler: BlockHandler::new(),

			sockname: Vec::new(),
		}))
	}

	/// Returns the socket's descriptor.
	#[inline(always)]
	pub fn desc(&self) -> &SocketDesc {
		&self.desc
	}

	/// Returns the socket's transmit pipeline.
	#[inline(always)]
	pub fn transmit_pipeline(&self) -> Option<&osi::TransmitPipeline<'static>> {
		self.transmit_pipeline.as_ref()
	}

	/// Reads the given socket option.
	///
	/// Arguments:
	/// - `level` is the level (protocol) at which the option is located.
	/// - `optname` is the name of the option.
	/// - `optval` is the value of the option.
	///
	/// The function returns a value to be returned by the syscall on success.
	pub fn get_opt(
		&self,
		_level: c_int,
		_optname: c_int,
		_optval: &mut [u8],
	) -> Result<c_int, Errno> {
		// TODO
		todo!()
	}

	/// Writes the given socket option.
	///
	/// Arguments:
	/// - `level` is the level (protocol) at which the option is located.
	/// - `optname` is the name of the option.
	/// - `optval` is the value of the option.
	///
	/// The function returns a value to be returned by the syscall on success.
	pub fn set_opt(
		&mut self,
		_level: c_int,
		_optname: c_int,
		_optval: &[u8],
	) -> Result<c_int, Errno> {
		// TODO
		Ok(0)
	}

	/// Writes the bound socket name into `sockaddr`.
	/// If the buffer is too small, the address is truncated.
	///
	/// The function returns the length of the socket address.
	pub fn read_sockname(&self, sockaddr: &mut [u8]) -> usize {
		let len = min(sockaddr.len(), self.sockname.len());
		sockaddr[..len].copy_from_slice(&self.sockname);

		self.sockname.len()
	}

	/// Tells whether the socket is bound.
	pub fn is_bound(&self) -> bool {
		!self.sockname.is_empty()
	}

	/// Binds the socket to the given address.
	///
	/// `sockaddr` is the new socket name.
	///
	/// If the socket is already bound, or if the address is invalid, or if the address is already
	/// in used, the function returns an error.
	pub fn bind(&mut self, sockaddr: &[u8]) -> EResult<()> {
		if self.is_bound() {
			return Err(errno!(EINVAL));
		}
		// TODO check if address is already in used (EADDRINUSE)
		// TODO check the requested network interface exists (EADDRNOTAVAIL)
		// TODO check address against stack's domain

		self.sockname = Vec::from_slice(sockaddr)?;
		Ok(())
	}

	// TODO add support for msghdr
	/// Sends a packet with the specified pipeline.
	///
	/// Arguments:
	/// - `buf` is the buffer with the data
	/// - `pipeline` is the transmit pipeline to use.
	///
	/// On success, the function returns the number of bytes sent.
	pub fn send_with_pipeline(
		&mut self,
		buf: &[u8],
		pipeline: &TransmitPipeline<'_>,
	) -> EResult<usize> {
		pipeline.transmit(BuffList::from(buf))?;
		Ok(buf.len())
	}

	/// Shuts down the receive side of the socket.
	pub fn shutdown_receive(&mut self) {
		self.receive_buffer = None;
	}

	/// Shuts down the transmit side of the socket.
	pub fn shutdown_transmit(&mut self) {
		self.transmit_buffer = None;
	}
}

impl TryDefault for Socket {
	fn try_default() -> EResult<Self> {
		let desc = SocketDesc {
			domain: SocketDomain::AfUnix,
			type_: SocketType::SockRaw,
			protocol: 0,
		};

		Ok(Self {
			desc,
			transmit_pipeline: None,

			receive_buffer: Some(RingBuffer::new(crate::vec![0; BUFFER_SIZE]?)),
			transmit_buffer: Some(RingBuffer::new(crate::vec![0; BUFFER_SIZE]?)),

			open_count: 0,

			block_handler: BlockHandler::new(),

			sockname: Default::default(),
		})
	}
}

impl Buffer for Socket {
	fn get_capacity(&self) -> usize {
		// TODO
		todo!()
	}

	fn increment_open(&mut self, _read: bool, _write: bool) {
		self.open_count += 1;
	}

	fn decrement_open(&mut self, _read: bool, _write: bool) {
		self.open_count -= 1;
		if self.open_count == 0 {
			// TODO close the socket
		}
	}

	fn add_waiting_process(&mut self, proc: &mut Process, mask: u32) -> Result<(), Errno> {
		self.block_handler.add_waiting_process(proc, mask)
	}

	fn ioctl(
		&mut self,
		_mem_space: Arc<IntMutex<MemSpace>>,
		_request: ioctl::Request,
		_argp: *const c_void,
	) -> Result<u32, Errno> {
		// TODO
		todo!();
	}
}

impl IO for Socket {
	fn get_size(&self) -> u64 {
		0
	}

	/// Note: This implemention ignores the offset.
	fn read(&mut self, _: u64, _buf: &mut [u8]) -> Result<(u64, bool), Errno> {
		if !self.desc.type_.is_stream() {
			// TODO error
		}

		// TODO
		todo!();
	}

	/// Note: This implemention ignores the offset.
	fn write(&mut self, _: u64, _buf: &[u8]) -> Result<u64, Errno> {
		// A destination address is required
		let Some(_pipeline) = self.transmit_pipeline.as_ref() else {
			return Err(errno!(EDESTADDRREQ));
		};

		// TODO
		todo!()
	}

	fn poll(&mut self, _mask: u32) -> Result<u32, Errno> {
		// TODO
		todo!();
	}
}
