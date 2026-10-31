// SPDX-License-Identifier: MPL-2.0
//! Integrated Arthur-on-Atrium compositor session driver.

use libneo::atrium_protocol::{
    Header, OP_COMMIT, OP_CREATE_SURFACE, OP_DESTROY_SURFACE, OP_FOCUS_REQUEST,
    OP_IMPORT_ATTACH_BUFFER, REPLY_LEN, ROLE_TEXT, Reply, Request, SurfaceRegistry, decode_request,
    encode_reply,
};
#[cfg(any(
    feature = "arthur-compositor-baremetal",
    feature = "product-integration"
))]
use libneo::sys_input_focus_activate;
#[cfg(feature = "product-integration")]
use libneo::sys_input_focus_test_inject;
use libneo::{
    ATRIUM_FOCUS_ACTIVATE, ATRIUM_MAP_READ, ATRIUM_PRESENT, ATRIUM_QUERY, BUFFER_DESC_V1_LEN,
    CapRights, EPERM, FULL_CHANNEL_RIGHTS, Handle, INPUT_EVENT_V1_LEN, PRESENT_RESULT_V1_LEN,
    encode_buffer_desc, meta_rights, sys_buffer_map, sys_buffer_query, sys_chan_create,
    sys_fb_present, sys_grant, sys_input_focus_recv, sys_present_fence_wait, sys_recv,
    sys_recv_grant, sys_send, sys_wait, sys_write,
};
#[cfg(feature = "arthur-memory")]
use libneo::{FileRights, OPEN_ROOT_TEMPORARY, sys_close, sys_open};

const NS: u64 = 0x4152_5448_5552_0001;
const SURFACE_CAP_PROVENANCE: u64 = 0x5355_5246_4152_0001;
const BUFFER_CAP_PROVENANCE: u64 = 0x4255_4646_4152_0001;
const WIDTH: u32 = 640;
const HEIGHT: u32 = 400;
const STRIDE: u32 = WIDTH * 4;
const BYTES: u64 = STRIDE as u64 * HEIGHT as u64;
const DST: (usize, usize) = (64, 64);
const ANSWER_SERIAL: u64 = 0x4152_5448_5552_0001;
#[cfg(feature = "arthur-memory")]
const MEMORY_DIR: &str = "/neofs/.arthur/memory";

pub fn run() -> i32 {
    log(b"SHOWCASE boot framebuffer=ready owner=g1-compositor\n");
    log(b"SHOWCASE compositor up protocol=ADR-073 input=ADR-074\n");

    let (session_server, session_client) = match sys_chan_create() {
        Ok(pair) => pair,
        Err(_) => return 60,
    };
    #[cfg(feature = "arthur-memory")]
    let (_pid, process) = {
        match sys_open(
            MEMORY_DIR,
            OPEN_ROOT_TEMPORARY,
            FileRights::LOOKUP.union(FileRights::READDIR),
        ) {
            Ok(memory) => {
                let (_slot1_parent, slot1_child) = match sys_chan_create() {
                    Ok(pair) => pair,
                    Err(_) => {
                        let _ = sys_close(memory);
                        return 89;
                    }
                };
                let (_slot2_parent, slot2_child) = match sys_chan_create() {
                    Ok(pair) => pair,
                    Err(_) => {
                        let _ = sys_close(memory);
                        return 90;
                    }
                };
                let inherited = [session_client, slot1_child, slot2_child, Handle(memory.0)];
                let spawned =
                    match libneo::sys_spawn_scoped("arthur", &inherited, libneo::ARTHUR_SCOPES) {
                        Ok(spawned) => spawned,
                        Err(_) => {
                            let _ = sys_close(memory);
                            return 61;
                        }
                    };
                let _ = sys_close(memory);
                spawned
            }
            Err(_) => {
                log(b"SHOWCASE memory mode=ephemeral reason=missing-neofs-arthur-memory\n");
                let inherited = [session_client];
                match libneo::sys_spawn_scoped("arthur", &inherited, libneo::ARTHUR_SCOPES) {
                    Ok(spawned) => spawned,
                    Err(_) => return 61,
                }
            }
        }
    };
    #[cfg(not(feature = "arthur-memory"))]
    let (_pid, process) = {
        let inherited = [session_client];
        match libneo::sys_spawn_scoped("arthur", &inherited, libneo::ARTHUR_SCOPES) {
            Ok(spawned) => spawned,
            Err(_) => return 61,
        }
    };

    let mut registry = SurfaceRegistry::new(NS, WIDTH, HEIGHT);
    let (create_header, create) = match recv_request(session_server) {
        Ok((header, Request::Create(create))) if create.role == ROLE_TEXT => (header, create),
        _ => return 62,
    };
    let (surface_server, surface_client) = match sys_chan_create() {
        Ok(pair) => pair,
        Err(_) => return 63,
    };
    let identity = match registry.create(create, SURFACE_CAP_PROVENANCE) {
        Ok(identity) => identity,
        Err(_) => return 64,
    };
    if !send_reply(
        session_server,
        OP_CREATE_SURFACE,
        create_header.request_id,
        0,
        identity,
    ) || !grant_surface(session_server, surface_client)
    {
        return 65;
    }
    log(b"SHOWCASE surface created role=TEXT client=arthur cap=dedicated-channel\n");
    if !recv_control(session_server, b'B') {
        return 88;
    }
    log(
        b"SHOWCASE bypass client_present=EPERM buffer=self-created-fresh authority=gpu-submit client_cap=absent\n",
    );

    let (import_header, import) = match recv_request(surface_server) {
        Ok((header, Request::ImportAttach(import))) => (header, import),
        _ => return 66,
    };
    if sys_send(surface_server, b"R") != 1 {
        return 67;
    }
    let compositor_rights = CapRights::new(
        meta_rights::USE,
        ATRIUM_MAP_READ | ATRIUM_QUERY | ATRIUM_PRESENT,
    );
    let buffer = match sys_recv_grant(surface_server, compositor_rights) {
        Ok(buffer) => buffer,
        Err(_) => return 68,
    };
    let mut desc = [0u8; BUFFER_DESC_V1_LEN];
    let expected_desc =
        encode_buffer_desc(WIDTH, HEIGHT, STRIDE, libneo::FORMAT_XRGB8888_LE, BYTES);
    if sys_buffer_query(buffer, &mut desc) != 0
        || desc != expected_desc
        || sys_buffer_map(buffer, 0).is_ok()
        || sys_buffer_map(buffer, 1).is_err()
        || registry
            .import_attach_on_channel(
                import,
                identity.surface_cap_provenance,
                BUFFER_CAP_PROVENANCE,
            )
            .is_err()
        || !send_reply(
            surface_server,
            OP_IMPORT_ATTACH_BUFFER,
            import_header.request_id,
            0,
            identity,
        )
    {
        return 69;
    }
    log(b"SHOWCASE seam buffer=client-rw compositor-ro-nx attach=accepted\n");

    let initial_serial =
        match recv_commit_and_present(surface_server, buffer, &mut registry, identity) {
            Ok(serial) => serial,
            Err(code) => return code,
        };
    if initial_serial != 1 {
        return 70;
    }
    log(
        b"SHOWCASE scanout compositor_present=allowed authority=gpu-submit compositor_cap=held backend=software\n",
    );

    let (focus_header, focus) = match recv_request(surface_server) {
        Ok((header, Request::Focus(focus))) => (header, focus),
        _ => return 71,
    };
    if registry.focus_request_on_channel(
        focus,
        identity.surface_cap_provenance,
        |surface_id, role| surface_id == identity.surface_id && role == ROLE_TEXT,
    ) != Ok(true)
        || !send_reply(
            surface_server,
            OP_FOCUS_REQUEST,
            focus_header.request_id,
            0,
            identity,
        )
    {
        return 72;
    }
    let activate_rights = CapRights::new(meta_rights::USE, ATRIUM_FOCUS_ACTIVATE);
    let activator = match sys_recv_grant(session_server, activate_rights) {
        Ok(activator) => activator,
        Err(_) => return 73,
    };
    let mut poison = [0xA5; INPUT_EVENT_V1_LEN];
    if sys_input_focus_recv(activator, &mut poison) != EPERM || poison != [0xA5; INPUT_EVENT_V1_LEN]
    {
        return 74;
    }
    if !recv_control(session_server, b'G') {
        return 75;
    }
    log(b"SHOWCASE greeting composited surface=arthur compositor_event_bytes=0\n");

    if !recv_control(session_server, b'M') {
        return 76;
    }
    log(b"SHOWCASE model resident with_compositor=true source=neofs\n");

    #[cfg(not(any(
        feature = "arthur-compositor-baremetal",
        feature = "product-integration"
    )))]
    {
        log(b"SHOWCASE slice=A greeting=composited qemu_gate=ready\n");
        if sys_send(session_server, b"Q") != 1
            || handle_destroy(surface_server, &mut registry, identity).is_err()
        {
            return 77;
        }
    }

    #[cfg(any(
        feature = "arthur-compositor-baremetal",
        feature = "product-integration"
    ))]
    {
        if sys_input_focus_activate(activator) != 0 || sys_send(session_server, b"K") != 1 {
            return 78;
        }
        log(b"SHOWCASE focus activated surface=arthur no_snoop=true\n");
        #[cfg(feature = "product-integration")]
        {
            // "hello world\n" as USB-HID usage IDs. These enter at the same
            // test-only router seam DW-1 uses, after the real InputFocus leaf
            // is active; Arthur must receive and render every decoded byte.
            const PROMPT: &[u8] = &[
                0x0b, 0x08, 0x0f, 0x0f, 0x12, 0x2c, 0x1a, 0x12, 0x15, 0x0f, 0x07, 0x28,
            ];
            for &keycode in PROMPT {
                if sys_input_focus_test_inject(keycode, 0) != 0 {
                    return 91;
                }
            }
            log(b"SHOWCASE product typed question=hello_world bytes=12 source=input_focus_test_router\n");
        }
        #[cfg(feature = "arthur-compositor-baremetal")]
        log(
            b"SHOWCASE keyboard loop=armed source=physical-usb-hid qemu=unsupported fail_closed=wait-for-hid\n",
        );
        loop {
            let serial =
                match recv_commit_and_present(surface_server, buffer, &mut registry, identity) {
                    Ok(serial) => serial,
                    Err(code) => return code,
                };
            // Arthur commits the rect it drew (Desktop S2), not the full
            // surface; the kernel validates and blits exactly that rect.
            log(b"SHOWCASE present commit=accepted damage=client-rect\n");
            if serial == ANSWER_SERIAL {
                log(
                    b"SHOWCASE answer composited surface=arthur forward=real serial=0x4152544855520001\n",
                );
                if handle_destroy(surface_server, &mut registry, identity).is_err() {
                    return 79;
                }
                break;
            }
        }
    }

    match sys_wait(process, 0) {
        Ok(0) => {
            #[cfg(not(any(
                feature = "arthur-compositor-baremetal",
                feature = "product-integration"
            )))]
            log(b"SHOWCASE slice=A pass render_seam=proven input=not_exercised\n");
            #[cfg(feature = "arthur-compositor-baremetal")]
            log(b"SHOWCASE slice=B pass keyboard_to_answer=complete proof=joint-silicon\n");
            #[cfg(feature = "product-integration")]
            log(b"SHOWCASE product arthur pass typed_question_to_composited_answer=complete\n");
            0
        }
        _ => 80,
    }
}

fn recv_commit_and_present(
    surface: Handle,
    buffer: Handle,
    registry: &mut SurfaceRegistry,
    identity: libneo::atrium_protocol::Identity,
) -> Result<u64, i32> {
    let (header, commit) = match recv_request(surface) {
        Ok((header, Request::Commit(commit))) => (header, commit),
        _ => return Err(81),
    };
    let serial = registry
        .commit_on_channel(commit, identity.surface_cap_provenance)
        .map_err(|_| 82)?;
    let epoch = sys_fb_present(buffer, DST.0, DST.1);
    if epoch < 1 {
        return Err(83);
    }
    let mut result = [0u8; PRESENT_RESULT_V1_LEN];
    if sys_present_fence_wait(buffer, epoch as u64, &mut result, 1000) != 0
        || u32::from_le_bytes([result[8], result[9], result[10], result[11]]) != 1
        || !send_reply(surface, OP_COMMIT, header.request_id, 0, identity)
    {
        return Err(84);
    }
    Ok(serial)
}

fn handle_destroy(
    surface: Handle,
    registry: &mut SurfaceRegistry,
    identity: libneo::atrium_protocol::Identity,
) -> Result<(), i32> {
    let (header, destroy) = match recv_request(surface) {
        Ok((header, Request::Destroy(destroy))) => (header, destroy),
        _ => return Err(85),
    };
    registry
        .destroy_on_channel(destroy, identity.surface_cap_provenance)
        .map_err(|_| 86)?;
    if registry.is_alive(identity) {
        return Err(87);
    }
    // Emit while Arthur is still blocked on the reply so the blind-hardware
    // transcript cannot byte-interleave this seam with its model-drop log.
    log(b"SHOWCASE surface destroyed client=arthur registry_alive=false\n");
    if !send_reply(surface, OP_DESTROY_SURFACE, header.request_id, 0, identity) {
        return Err(87);
    }
    Ok(())
}

fn recv_request(channel: Handle) -> Result<(Header, Request), i64> {
    let mut bytes = [0u8; libneo::atrium_protocol::MAX_MESSAGE_LEN];
    let len = sys_recv(channel, &mut bytes);
    if len <= 0 {
        return Err(len);
    }
    decode_request(&bytes[..len as usize]).map_err(|_| libneo::EINVAL)
}

fn send_reply(
    channel: Handle,
    opcode: u16,
    request_id: u64,
    status: u32,
    identity: libneo::atrium_protocol::Identity,
) -> bool {
    let bytes = encode_reply(Reply {
        opcode,
        request_id,
        status,
        identity,
    });
    bytes.len() == REPLY_LEN && sys_send(channel, &bytes) == REPLY_LEN as i64
}

fn grant_surface(session: Handle, client_endpoint: Handle) -> bool {
    let mut byte = [0u8; 1];
    sys_send(session, b"O") == 1
        && sys_recv(session, &mut byte) == 1
        && byte[0] == b'R'
        && sys_grant(session, client_endpoint, FULL_CHANNEL_RIGHTS) == 0
        && sys_recv(session, &mut byte) == 1
        && byte[0] == b'A'
}

fn recv_control(session: Handle, expected: u8) -> bool {
    let mut byte = [0u8; 1];
    sys_recv(session, &mut byte) == 1 && byte[0] == expected
}

fn log(message: &[u8]) {
    let _ = sys_write(1, message.as_ptr(), message.len());
}
