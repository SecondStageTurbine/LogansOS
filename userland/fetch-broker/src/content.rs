// SPDX-License-Identifier: MPL-2.0
//! OT-61 rung 5c: the browser's content process, for the `fetch_broker`
//! scenario (`docs/THINK-2026-10-30-ot61-r5-fetch-broker.md` §7.1).
//!
//! Spawned with exactly one handle, handle 0: its channel to the fetch broker.
//! It holds no dial capability, and it proves that before it fetches anything
//! (#29 review item 1): a dial with no cap, and with each handle number it
//! might be handed, is refused. Then it asks the broker for four URLs and
//! checks each answer. The exit code is 0 on success, else the number of the
//! check that failed.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use fetch_proto::wire::{self, ErrorCode, Method, Msg};
use libneo::{
    EACCES, Handle, NO_NET_DIAL_CAP, NetDialCap, sys_exit, sys_net_dial, sys_recv, sys_send,
    sys_write,
};

const BROKER: Handle = Handle(0);

fn say(msg: &str) {
    let _ = sys_write(1, msg.as_ptr(), msg.len());
}

fn fail(code: i32) -> ! {
    say("[ fetch-content ] FAIL\n");
    sys_exit(code);
}

/// What one request produced.
enum Outcome {
    Body { status: u16, body: Vec<u8> },
    Error(ErrorCode),
}

fn fetch(id: u32, url: &str) -> Result<Outcome, i32> {
    let req = wire::encode(&Msg::Fetch {
        id,
        method: Method::Get,
        url: String::from(url),
        headers: Vec::new(),
    })
    .map_err(|_| 101)?;
    if sys_send(BROKER, &req) < 0 {
        return Err(102);
    }
    // One byte past the longest message: an over-long reply is refused
    // whole, never cut to a prefix that decodes (`wire::MAX_MESSAGE`).
    let mut buf = vec![0u8; wire::RECV_BUF];
    let mut status = 0;
    let mut body = Vec::new();
    loop {
        let n = sys_recv(BROKER, &mut buf);
        if n <= 0 {
            return Err(103);
        }
        match wire::decode(&buf[..n as usize]).map_err(|_| 104)? {
            Msg::Head {
                id: i, status: s, ..
            } if i == id => status = s,
            Msg::Body { id: i, data } if i == id => body.extend_from_slice(&data),
            Msg::End { id: i } if i == id => return Ok(Outcome::Body { status, body }),
            Msg::Error { id: i, code } if i == id => return Ok(Outcome::Error(code)),
            _ => return Err(105),
        }
    }
}

/// The content process can't reach the network itself: no cap is `EACCES`,
/// and neither its only handle (the channel) nor the next handle numbers are
/// a dial cap. Each of these dials would reach the fixture with the root cap
/// the launcher holds.
fn direct_dial_refused() -> bool {
    let host = libneo::tls::PHASE4_TEST_HOST;
    let port = libneo::tls::PHASE4_TEST_PORT;
    if sys_net_dial(NO_NET_DIAL_CAP, host, port) != Err(EACCES) {
        return false;
    }
    (0..4).all(|h| sys_net_dial(NetDialCap(Handle(h)), host, port).is_err())
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    say("[ fetch-content ] up, holding only the broker channel\n");
    if !direct_dial_refused() {
        fail(80);
    }
    // A private address literal: refused by the broker's policy.
    match fetch(1, "https://127.0.0.1:4433/") {
        Ok(Outcome::Error(ErrorCode::PolicyRefused)) => {}
        Ok(_) => fail(86),
        Err(code) => fail(code),
    }
    // Cleartext: refused by policy.
    match fetch(2, "http://phase4.test:4433/") {
        Ok(Outcome::Error(ErrorCode::PolicyRefused)) => {}
        Ok(_) => fail(87),
        Err(code) => fail(code),
    }
    // A name that resolves to loopback, outside the fixture allowance: the
    // broker's narrowed cap is refused by the kernel's dial (5b).
    match fetch(3, "https://rebind.test:4433/") {
        Ok(Outcome::Error(ErrorCode::PolicyRefused)) => {}
        Ok(_) => fail(88),
        Err(code) => fail(code),
    }
    // The fixture, through the broker: TLS in the broker, the body here.
    match fetch(4, "https://phase4.test:4433/") {
        Ok(Outcome::Body { status: 200, body }) if body == libneo::tls::PHASE4_TEST_BODY => {}
        Ok(Outcome::Body { .. }) => fail(89),
        Ok(Outcome::Error(code)) => {
            let c = [b'0' + code as u8];
            say("[ fetch-content ] fixture error code=");
            say(core::str::from_utf8(&c).unwrap_or("?"));
            say("\n");
            fail(90);
        }
        Err(code) => fail(code),
    }
    say(
        "[ fetch-content ] direct_dial=refused private_literal=refused cleartext=refused \
         loopback_name=kernel_refused https_body=ok\n",
    );
    // Exiting closes the channel, which ends the broker.
    sys_exit(0);
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    say("[ fetch-content ] panic\n");
    sys_exit(101);
}
