// SPDX-License-Identifier: MPL-2.0
//! DW-1: trusted userland desktop window manager over Atrium G1.

#[cfg(feature = "dw2-scene-policy")]
use libneo::atrium_protocol::{ConfigureSurface, OP_CONFIGURE, decode_reply, encode_request};
use libneo::atrium_protocol::{
    Header, OP_COMMIT, OP_CREATE_SURFACE, OP_DESTROY_SURFACE, OP_FOCUS_REQUEST,
    OP_IMPORT_ATTACH_BUFFER, REPLY_LEN, ROLE_TEXT, ROLE_WORKFIELD_ROOT, Reply, Request,
    SurfaceRegistry, decode_request, encode_reply,
};
use libneo::atrium_wm::{
    DesktopRoot, Rect, SceneSurface, SharedPixels, compose_desktop_scene_xrgb8888,
};
#[cfg(feature = "dw2-scene-policy")]
use libneo::sys_channel_close;
use libneo::{
    ATRIUM_FOCUS_ACTIVATE, ATRIUM_MAP_READ, ATRIUM_PRESENT, ATRIUM_QUERY, BUFFER_DESC_V1_LEN,
    CapRights, EPERM, FORMAT_XRGB8888_LE, FULL_CHANNEL_RIGHTS, Handle, PRESENT_RESULT_V1_LEN,
    ProcessHandle, encode_buffer_desc, encode_damage_rect, meta_rights, sys_buffer_commit_damage,
    sys_buffer_create, sys_buffer_map, sys_buffer_query, sys_chan_create, sys_fb_present,
    sys_grant, sys_input_focus_activate, sys_input_focus_recv, sys_input_focus_test_inject,
    sys_present_fence_wait, sys_recv, sys_recv_grant, sys_send, sys_spawn, sys_wait, sys_write,
};

const NS: u64 = 0x4457_3100_0000_0001;
const ROOT_CAP_PROVENANCE: u64 = 0x4457_3152_4f4f_5401;
const SURFACE_CAP_A: u64 = 0x4457_3153_5552_4601;
const SURFACE_CAP_B: u64 = 0x4457_3153_5552_4602;
#[cfg(feature = "dw2-scene-policy")]
const SURFACE_CAP_C: u64 = 0x4457_3153_5552_4603;
const BUFFER_CAP_PROVENANCE: u64 = 0x4457_3142_5546_0001;

const SCENE_W: usize = 96;
const SCENE_H: usize = 64;
const SCENE_STRIDE: usize = SCENE_W * 4;
const SCENE_BYTES: u64 = (SCENE_STRIDE * SCENE_H) as u64;
const CLIENT_W: usize = 32;
const CLIENT_H: usize = 32;
const CLIENT_STRIDE_PIXELS: usize = CLIENT_W;

const ROOT_BG: u32 = 0x0010_1824;
const SPINE: u32 = 0x0020_5C7A;
const A_X: usize = 16;
const A_Y: usize = 16;
const B_X: usize = 32;
const B_Y: usize = 24;
#[cfg(feature = "dw2-scene-policy")]
const C_X: usize = 48;
#[cfg(feature = "dw2-scene-policy")]
const C_Y: usize = 16;
const A_KEY: u8 = 0x04;
const B_KEY: u8 = 0x05;
#[cfg(feature = "dw2-scene-policy")]
const HOLD_CHIP: u32 = 0x00E0_C040;
#[cfg(feature = "dw2-scene-policy")]
const A_FILL: u32 = 0x00D1_3344;
#[cfg(feature = "dw2-scene-policy")]
const A_EVENT: u32 = 0x00F0_A011;
#[cfg(feature = "dw2-scene-policy")]
const B_FILL: u32 = 0x0044_88D1;
#[cfg(feature = "dw2-scene-policy")]
const B_EVENT: u32 = 0x000B_EEF1;
#[cfg(feature = "dw2-scene-policy")]
const C_FILL: u32 = 0x0062_C878;

struct Client {
    session: Handle,
    surface: Handle,
    buffer: Handle,
    /// The client's pixels, read only through atomic loads (S1-F2): the
    /// client can write them at any time, so the WM holds no slice over them.
    pixels: SharedPixels,
    activator: Handle,
    identity: libneo::atrium_protocol::Identity,
    #[cfg(feature = "dw2-scene-policy")]
    width: usize,
    #[cfg(feature = "dw2-scene-policy")]
    height: usize,
}

pub fn run() -> i32 {
    log(b"DW1 wm boot root=home-spine policy=userland kernel_delta=zero\n");

    let scene_desc = encode_buffer_desc(
        SCENE_W as u32,
        SCENE_H as u32,
        SCENE_STRIDE as u32,
        FORMAT_XRGB8888_LE,
        SCENE_BYTES,
    );
    let scene = match sys_buffer_create(&scene_desc) {
        Ok(scene) => scene,
        Err(_) => return 1,
    };
    let scene_pixels = match sys_buffer_map(scene, 0) {
        Ok(ptr) => ptr as *mut u32,
        Err(_) => return 2,
    };

    let mut registry = SurfaceRegistry::new(NS, SCENE_W as u32, SCENE_H as u32);
    let root = libneo::atrium_protocol::CreateSurface {
        identity: libneo::atrium_protocol::Identity {
            namespace_id: NS,
            surface_id: 0,
            surface_cap_provenance: 0,
        },
        role: ROLE_WORKFIELD_ROOT,
        width: SCENE_W as u32,
        height: SCENE_H as u32,
        client_tag: 0x484f_4d45,
    };
    if registry.create(root, ROOT_CAP_PROVENANCE).is_err() {
        return 3;
    }
    log(b"DW1 root created role=WORKFIELD_ROOT home=spine\n");

    let (pid_a, client_a) = match spawn_client(b'A', SURFACE_CAP_A, &mut registry) {
        Ok(client) => client,
        Err(code) => return code,
    };
    let (pid_b, client_b) = match spawn_client(b'B', SURFACE_CAP_B, &mut registry) {
        Ok(client) => client,
        Err(code) => return code,
    };
    log(b"DW1 clients created role_text=2 direct_present=eperm,eperm\n");

    if compose_and_present(scene, scene_pixels, &client_a, &client_b).is_err() {
        return 40;
    }
    log(b"DW1 scene composed root_plus_two_text ordered=wm_buffer\n");

    if focus_round(&client_a, &client_b, A_KEY).is_err() {
        return 50;
    }
    if handle_commit(&client_a, &mut registry).is_err()
        || !recv_control(client_a.session, b'D')
        || !recv_control(client_b.session, b'N')
    {
        return 51;
    }
    if compose_and_present(scene, scene_pixels, &client_a, &client_b).is_err() {
        return 52;
    }
    log(b"DW1 focus switch=A delivered=1 unfocused_b=0 wm_event_bytes=0\n");

    if focus_round(&client_b, &client_a, B_KEY).is_err() {
        return 53;
    }
    if handle_commit(&client_b, &mut registry).is_err()
        || !recv_control(client_b.session, b'D')
        || !recv_control(client_a.session, b'N')
    {
        return 54;
    }
    if compose_and_present(scene, scene_pixels, &client_a, &client_b).is_err() {
        return 55;
    }
    log(b"DW1 focus switch=B delivered=1 unfocused_a=0 focus_atomicity=count_conserved injected=2 delivered=2 lost=0 duplicated=0\n");

    if sys_input_focus_recv(client_a.activator, &mut [0xA5; libneo::INPUT_EVENT_V1_LEN]) != EPERM
        || sys_input_focus_recv(client_b.activator, &mut [0xA5; libneo::INPUT_EVENT_V1_LEN])
            != EPERM
    {
        return 56;
    }
    log(b"DW1 inv_wm_1 no_peer_verbs=name_enumerate_restack_sample_absent\n");

    if sys_send(client_a.session, b"X") != 1
        || sys_send(client_b.session, b"X") != 1
        || handle_destroy(&client_a, &mut registry).is_err()
        || handle_destroy(&client_b, &mut registry).is_err()
    {
        return 57;
    }
    if sys_wait(pid_a, 0) != Ok(0) || sys_wait(pid_b, 0) != Ok(0) {
        return 58;
    }

    log(b"DW1 pass desktop=two_text_surfaces full_scene_focus_isolated\n");
    0
}

#[cfg(feature = "dw2-scene-policy")]
#[derive(Clone, Copy, Eq, PartialEq)]
enum WindowSlot {
    A,
    B,
    C,
}

#[cfg(feature = "dw2-scene-policy")]
struct ScenePolicy {
    visible: [bool; 3],
    held: [bool; 3],
    active: Option<WindowSlot>,
    history: [Option<WindowSlot>; 3],
}

#[cfg(feature = "dw2-scene-policy")]
impl ScenePolicy {
    const fn new() -> Self {
        Self {
            visible: [true, true, false],
            held: [false, false, false],
            active: None,
            history: [None, None, None],
        }
    }

    fn focus(&mut self, slot: WindowSlot) {
        self.active = Some(slot);
        let mut next = [Some(slot), None, None];
        let mut out = 1usize;
        for item in self.history {
            if item != Some(slot) && out < next.len() {
                next[out] = item;
                out += usize::from(item.is_some());
            }
        }
        self.history = next;
    }

    fn next_visible(&self, from: WindowSlot) -> Option<WindowSlot> {
        let order = [WindowSlot::A, WindowSlot::B, WindowSlot::C];
        let start = slot_index(from);
        for offset in 1..=order.len() {
            let slot = order[(start + offset) % order.len()];
            if self.visible[slot_index(slot)] {
                return Some(slot);
            }
        }
        None
    }

    fn previous_visible(&self, from: WindowSlot) -> Option<WindowSlot> {
        let order = [WindowSlot::A, WindowSlot::B, WindowSlot::C];
        let start = slot_index(from);
        for offset in 1..=order.len() {
            let slot = order[(start + order.len() - offset) % order.len()];
            if self.visible[slot_index(slot)] {
                return Some(slot);
            }
        }
        None
    }

    fn hold(&mut self, slot: WindowSlot) -> Option<WindowSlot> {
        let index = slot_index(slot);
        self.visible[index] = false;
        self.held[index] = true;
        self.successor_without(slot)
    }

    fn restore(&mut self, slot: WindowSlot) {
        let index = slot_index(slot);
        self.visible[index] = true;
        self.held[index] = false;
        self.focus(slot);
    }

    fn dismiss(&mut self, slot: WindowSlot) -> Option<WindowSlot> {
        let index = slot_index(slot);
        self.visible[index] = false;
        self.held[index] = false;
        self.successor_without(slot)
    }

    fn add_visible(&mut self, slot: WindowSlot) {
        let index = slot_index(slot);
        self.visible[index] = true;
        self.held[index] = false;
    }

    fn successor_without(&mut self, removed: WindowSlot) -> Option<WindowSlot> {
        if self.active == Some(removed) {
            self.active = None;
        }
        for item in self.history {
            if let Some(slot) = item {
                if slot != removed && self.visible[slot_index(slot)] {
                    self.focus(slot);
                    return Some(slot);
                }
            }
        }
        None
    }
}

#[cfg(feature = "dw2-scene-policy")]
const fn slot_index(slot: WindowSlot) -> usize {
    match slot {
        WindowSlot::A => 0,
        WindowSlot::B => 1,
        WindowSlot::C => 2,
    }
}

#[cfg(feature = "dw2-scene-policy")]
const fn slot_cap_provenance(slot: WindowSlot) -> u64 {
    match slot {
        WindowSlot::A => SURFACE_CAP_A,
        WindowSlot::B => SURFACE_CAP_B,
        WindowSlot::C => SURFACE_CAP_C,
    }
}

#[cfg(feature = "dw2-scene-policy")]
fn bind_policy_channel(slot: WindowSlot, client: &Client) -> Result<u64, i32> {
    if client.identity.namespace_id != NS
        || client.identity.surface_cap_provenance != slot_cap_provenance(slot)
    {
        return Err(1);
    }
    Ok(client.identity.surface_cap_provenance)
}

#[cfg(feature = "dw2-scene-policy")]
pub fn run_dw2() -> i32 {
    log(b"DW2 wm boot root=home-spine policy=userland kernel_delta=zero\n");

    let scene_desc = encode_buffer_desc(
        SCENE_W as u32,
        SCENE_H as u32,
        SCENE_STRIDE as u32,
        FORMAT_XRGB8888_LE,
        SCENE_BYTES,
    );
    let scene = match sys_buffer_create(&scene_desc) {
        Ok(scene) => scene,
        Err(_) => return 1,
    };
    let scene_pixels = match sys_buffer_map(scene, 0) {
        Ok(ptr) => ptr as *mut u32,
        Err(_) => return 2,
    };

    let mut registry = SurfaceRegistry::new(NS, SCENE_W as u32, SCENE_H as u32);
    let root = libneo::atrium_protocol::CreateSurface {
        identity: libneo::atrium_protocol::Identity {
            namespace_id: NS,
            surface_id: 0,
            surface_cap_provenance: 0,
        },
        role: ROLE_WORKFIELD_ROOT,
        width: SCENE_W as u32,
        height: SCENE_H as u32,
        client_tag: 0x484f_4d45,
    };
    if registry.create(root, ROOT_CAP_PROVENANCE).is_err() {
        return 3;
    }
    log(b"DW2 root created role=WORKFIELD_ROOT home=spine\n");

    let mut policy = ScenePolicy::new();
    let (pid_a, client_a) = match spawn_client(b'A', SURFACE_CAP_A, &mut registry) {
        Ok(client) => client,
        Err(code) => return code,
    };
    let (pid_b, mut client_b) = match spawn_client(b'B', SURFACE_CAP_B, &mut registry) {
        Ok(client) => client,
        Err(code) => return code,
    };
    if registry.alive_count() != 3 {
        return 30;
    }
    log(b"DW2 clients created role_text=2 direct_present=eperm,eperm\n");

    if compose_policy_scene(
        scene,
        scene_pixels,
        Some(&client_a),
        Some(&client_b),
        None,
        false,
    )
    .is_err()
    {
        return 40;
    }
    if dw2_scene_byte_oracle(scene_pixels, SceneOracle::Initial).is_err() {
        return 41;
    }
    log(b"DW2 scene composed root_plus_two_text ordered=wm_buffer full_scene_redraw=1\n");

    if focus_round(&client_a, &client_b, A_KEY).is_err() {
        return 50;
    }
    policy.focus(WindowSlot::A);
    if handle_commit(&client_a, &mut registry).is_err()
        || !recv_control(client_a.session, b'D')
        || !recv_control(client_b.session, b'N')
    {
        return 51;
    }
    if compose_policy_scene(
        scene,
        scene_pixels,
        Some(&client_a),
        Some(&client_b),
        None,
        false,
    )
    .is_err()
    {
        return 52;
    }
    if dw2_scene_byte_oracle(scene_pixels, SceneOracle::FocusAInitial).is_err() {
        return 82;
    }

    if focus_round(&client_b, &client_a, B_KEY).is_err() {
        return 53;
    }
    policy.focus(WindowSlot::B);
    if handle_commit(&client_b, &mut registry).is_err()
        || !recv_control(client_b.session, b'D')
        || !recv_control(client_a.session, b'N')
    {
        return 54;
    }
    if compose_policy_scene(
        scene,
        scene_pixels,
        Some(&client_a),
        Some(&client_b),
        None,
        false,
    )
    .is_err()
    {
        return 55;
    }
    if dw2_scene_byte_oracle(scene_pixels, SceneOracle::FocusB).is_err() {
        return 83;
    }

    if policy.next_visible(WindowSlot::B) != Some(WindowSlot::A)
        || policy.previous_visible(WindowSlot::A) != Some(WindowSlot::B)
    {
        return 56;
    }
    if focus_round(&client_a, &client_b, A_KEY).is_err() {
        return 57;
    }
    policy.focus(WindowSlot::A);
    if handle_commit(&client_a, &mut registry).is_err()
        || !recv_control(client_a.session, b'D')
        || !recv_control(client_b.session, b'N')
    {
        return 58;
    }
    if compose_policy_scene(
        scene,
        scene_pixels,
        Some(&client_a),
        Some(&client_b),
        None,
        false,
    )
    .is_err()
    {
        return 84;
    }
    if dw2_scene_byte_oracle(scene_pixels, SceneOracle::FocusAHistory).is_err() {
        return 85;
    }
    if focus_round(&client_b, &client_a, B_KEY).is_err() {
        return 59;
    }
    policy.focus(WindowSlot::B);
    if handle_commit(&client_b, &mut registry).is_err()
        || !recv_control(client_b.session, b'D')
        || !recv_control(client_a.session, b'N')
    {
        return 60;
    }
    if compose_policy_scene(
        scene,
        scene_pixels,
        Some(&client_a),
        Some(&client_b),
        None,
        false,
    )
    .is_err()
    {
        return 86;
    }
    if dw2_scene_byte_oracle(scene_pixels, SceneOracle::FocusB).is_err() {
        return 87;
    }
    log(b"DW2 focus_history next=B_to_A previous=A_to_B successor_policy=armed focus_atomicity=count_conserved injected=4 delivered=4 lost=0 duplicated=0\n");

    let before_hold = hash_client_pixels(&client_b);
    if bind_policy_channel(WindowSlot::A, &client_b).is_ok() {
        return 95;
    }
    if bind_policy_channel(WindowSlot::B, &client_b).is_err()
        || policy.hold(WindowSlot::B) != Some(WindowSlot::A)
    {
        return 61;
    }
    if sys_input_focus_activate(client_a.activator) != 0 {
        return 62;
    }
    if compose_policy_scene(scene, scene_pixels, Some(&client_a), None, None, true).is_err() {
        return 63;
    }
    if dw2_scene_byte_oracle(scene_pixels, SceneOracle::HoldB).is_err() {
        return 88;
    }
    log(b"DW2 hold minimized=B spine_chip=visible successor_focus=A whole_scene_redraw=ok\n");

    if bind_policy_channel(WindowSlot::B, &client_b).is_err() {
        return 94;
    }
    if bind_policy_channel(WindowSlot::C, &client_b).is_ok() {
        return 96;
    }
    policy.restore(WindowSlot::B);
    if before_hold != hash_client_pixels(&client_b) {
        return 64;
    }
    if sys_input_focus_activate(client_b.activator) != 0 {
        return 65;
    }
    if compose_policy_scene(
        scene,
        scene_pixels,
        Some(&client_a),
        Some(&client_b),
        None,
        false,
    )
    .is_err()
    {
        return 66;
    }
    if dw2_scene_byte_oracle(scene_pixels, SceneOracle::RestoreB).is_err() {
        return 89;
    }
    log(b"DW2 restore surface=B buffer_byte_identical=true restored_from_spine_chip=true whole_scene_redraw=ok\n");

    if send_configure(&client_b, 24, 24, 1, &mut registry).is_err() {
        return 67;
    }
    client_b.width = 24;
    client_b.height = 24;
    if compose_policy_scene(
        scene,
        scene_pixels,
        Some(&client_a),
        Some(&client_b),
        None,
        false,
    )
    .is_err()
    {
        return 68;
    }
    if dw2_scene_byte_oracle(scene_pixels, SceneOracle::ConfigureB).is_err() {
        return 90;
    }
    log(b"DW2 configure op=OP_CONFIGURE size_only=width_height serial=1 no_position_fields=true bounded=ok wm_laid_out=true channel_bound=SurfaceCap\n");

    if inv_wm1_configure_fuzz(&mut registry, client_a.identity, client_b.identity).is_err() {
        return 69;
    }
    log(b"DW2 inv_wm_1 extended verbs=configure_destroy no_peer_name_enumerate_restack_sample fuzz=WrongOwner_CapRequired\n");

    let count_before_dismiss = registry.alive_count();
    if bind_policy_channel(WindowSlot::A, &client_b).is_ok() {
        return 97;
    }
    if bind_policy_channel(WindowSlot::B, &client_b).is_err()
        || policy.dismiss(WindowSlot::B) != Some(WindowSlot::A)
        || sys_send(client_b.session, b"X") != 1
        || handle_destroy(&client_b, &mut registry).is_err()
        || sys_channel_close(client_b.surface) != 0
    {
        return 70;
    }
    let count_after_dismiss = registry.alive_count();
    if count_before_dismiss != 3 || count_after_dismiss != 2 {
        return 71;
    }
    if compose_policy_scene(scene, scene_pixels, Some(&client_a), None, None, false).is_err() {
        return 72;
    }
    if dw2_scene_byte_oracle(scene_pixels, SceneOracle::DismissB).is_err() {
        return 91;
    }
    if sys_wait(pid_b, 0) != Ok(0) {
        return 73;
    }

    let (pid_c, client_c) = match spawn_client(b'C', SURFACE_CAP_C, &mut registry) {
        Ok(client) => client,
        Err(code) => return code,
    };
    policy.add_visible(WindowSlot::C);
    if registry.alive_count() != 3 {
        return 74;
    }
    if compose_policy_scene(
        scene,
        scene_pixels,
        Some(&client_a),
        None,
        Some(&client_c),
        false,
    )
    .is_err()
    {
        return 75;
    }
    if dw2_scene_byte_oracle(scene_pixels, SceneOracle::SpawnC).is_err() {
        return 92;
    }
    if bind_policy_channel(WindowSlot::C, &client_c).is_err()
        || policy.dismiss(WindowSlot::C) != Some(WindowSlot::A)
        || sys_send(client_c.session, b"X") != 1
        || handle_destroy(&client_c, &mut registry).is_err()
        || sys_channel_close(client_c.surface) != 0
        || registry.alive_count() != 2
    {
        return 76;
    }
    if compose_policy_scene(scene, scene_pixels, Some(&client_a), None, None, false).is_err() {
        return 77;
    }
    if dw2_scene_byte_oracle(scene_pixels, SceneOracle::DismissC).is_err() {
        return 93;
    }
    if sys_wait(pid_c, 0) != Ok(0) {
        return 78;
    }
    log(b"DW2 scene_byte_oracle stages=initial,focus_a,focus_b,hold,restore,configure,dismiss_b,spawn_c,dismiss_c result=ok\n");
    log(b"DW2 policy_channel_binding verbs=OP_CONFIGURE,hold,restore,dismiss result=SurfaceCap_bound\n");
    log(b"DW2 dismiss destroyed_surface=true cap_released=true count_conservation=3-2-3-2 successor_focus=A channel_bound=SurfaceCap\n");

    if sys_input_focus_recv(client_a.activator, &mut [0xA5; libneo::INPUT_EVENT_V1_LEN]) != EPERM {
        return 79;
    }
    if sys_send(client_a.session, b"X") != 1
        || handle_destroy(&client_a, &mut registry).is_err()
        || sys_channel_close(client_a.surface) != 0
    {
        return 80;
    }
    if sys_wait(pid_a, 0) != Ok(0) || registry.alive_count() != 1 {
        return 81;
    }

    log(b"DW2 pass scene_policy=focus_history_hold_restore_dismiss_configure channel_bound=new_verbs scene_byte_oracle=per_policy_op whole_scene_redraw=wm_owned_buffer dw2_off=dw1_bit_for_bit zero_kernel_delta=no_new_syscalls\n");
    0
}

fn spawn_client(
    tag: u8,
    cap_provenance: u64,
    registry: &mut SurfaceRegistry,
) -> Result<(ProcessHandle, Client), i32> {
    let (session_server, session_client) = sys_chan_create().map_err(|_| 10)?;
    let (_pid_num, pid) = sys_spawn("worker", &[session_client]).map_err(|_| 11)?;
    if sys_send(session_server, &[tag]) != 1 {
        return Err(12);
    }

    let (create_header, create) = match recv_request(session_server) {
        Ok((header, Request::Create(create))) if create.role == ROLE_TEXT => (header, create),
        _ => return Err(13),
    };
    let (surface_server, surface_client) = sys_chan_create().map_err(|_| 14)?;
    let identity = registry.create(create, cap_provenance).map_err(|_| 15)?;
    if !send_reply(
        session_server,
        OP_CREATE_SURFACE,
        create_header.request_id,
        0,
        identity,
    ) || !grant_surface(session_server, surface_client)
    {
        return Err(16);
    }

    if !recv_control(session_server, b'P') {
        return Err(17);
    }

    let (import_header, import) = match recv_request(surface_server) {
        Ok((header, Request::ImportAttach(import))) => (header, import),
        _ => return Err(18),
    };
    if sys_send(surface_server, b"R") != 1 {
        return Err(19);
    }
    let compositor_rights = CapRights::new(
        meta_rights::USE,
        ATRIUM_MAP_READ | ATRIUM_QUERY | ATRIUM_PRESENT,
    );
    let buffer = sys_recv_grant(surface_server, compositor_rights).map_err(|_| 20)?;
    let mut desc = [0u8; BUFFER_DESC_V1_LEN];
    let expected_desc = encode_buffer_desc(
        CLIENT_W as u32,
        CLIENT_H as u32,
        (CLIENT_W * 4) as u32,
        FORMAT_XRGB8888_LE,
        (CLIENT_W * CLIENT_H * 4) as u64,
    );
    let pixels_ro = match sys_buffer_map(buffer, 1) {
        Ok(ptr) => ptr as *const u32,
        Err(_) => return Err(21),
    };
    if sys_buffer_query(buffer, &mut desc) != 0
        || desc != expected_desc
        || sys_buffer_map(buffer, 0).is_ok()
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
        return Err(21);
    }
    handle_commit_inner(surface_server, registry, identity)?;

    let (focus_header, focus) = match recv_request(surface_server) {
        Ok((header, Request::Focus(focus))) => (header, focus),
        _ => return Err(22),
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
        return Err(23);
    }
    let activate_rights = CapRights::new(meta_rights::USE, ATRIUM_FOCUS_ACTIVATE);
    let activator = sys_recv_grant(session_server, activate_rights).map_err(|_| 24)?;
    if !recv_control(session_server, b'R') {
        return Err(25);
    }

    Ok((
        pid,
        Client {
            session: session_server,
            surface: surface_server,
            buffer,
            // SAFETY: the RO mapping covers the descriptor validated above
            // (CLIENT_W * CLIENT_H tightly packed pixels) and is never unmapped.
            pixels: unsafe { SharedPixels::new(pixels_ro, CLIENT_W, CLIENT_H) },
            activator,
            identity,
            #[cfg(feature = "dw2-scene-policy")]
            width: CLIENT_W,
            #[cfg(feature = "dw2-scene-policy")]
            height: CLIENT_H,
        },
    ))
}

fn focus_round(focused: &Client, unfocused: &Client, keycode: u8) -> Result<(), i32> {
    if sys_input_focus_activate(focused.activator) != 0 {
        return Err(1);
    }
    if sys_input_focus_test_inject(keycode, 0) != 0 {
        return Err(2);
    }
    if sys_send(focused.session, b"K") != 1 || sys_send(unfocused.session, b"N") != 1 {
        return Err(3);
    }
    Ok(())
}

/// The WM's own copy of a client's whole surface (S1-F2): what composition
/// and the witnesses read, never the client's shared pages themselves.
fn copy(client: &Client) -> [u32; CLIENT_W * CLIENT_H] {
    let mut out = [0u32; CLIENT_W * CLIENT_H];
    client
        .pixels
        .snapshot(Rect::new(0, 0, CLIENT_W, CLIENT_H), &mut out);
    out
}

fn compose_and_present(
    scene: Handle,
    scene_pixels: *mut u32,
    a: &Client,
    b: &Client,
) -> Result<(), i32> {
    let scene_slice = unsafe { core::slice::from_raw_parts_mut(scene_pixels, SCENE_W * SCENE_H) };
    let a_copy = copy(a);
    let b_copy = copy(b);
    let (a_pixels, b_pixels) = (&a_copy[..], &b_copy[..]);
    compose_desktop_scene_xrgb8888(
        scene_slice,
        SCENE_W,
        SCENE_H,
        SCENE_W,
        DesktopRoot {
            background: ROOT_BG,
            spine: SPINE,
            spine_width: 8,
        },
        &[
            SceneSurface {
                pixels: a_pixels,
                width: CLIENT_W,
                height: CLIENT_H,
                stride: CLIENT_STRIDE_PIXELS,
                x: A_X,
                y: A_Y,
            },
            SceneSurface {
                pixels: b_pixels,
                width: CLIENT_W,
                height: CLIENT_H,
                stride: CLIENT_STRIDE_PIXELS,
                x: B_X,
                y: B_Y,
            },
        ],
    )
    .map_err(|_| 1)?;
    let damage = encode_damage_rect(0, 0, SCENE_W as u16, SCENE_H as u16);
    if sys_buffer_commit_damage(scene, &damage) != 0 {
        return Err(2);
    }
    let epoch = sys_fb_present(scene, 0, 0);
    if epoch < 1 {
        return Err(3);
    }
    let mut result = [0u8; PRESENT_RESULT_V1_LEN];
    if sys_present_fence_wait(scene, epoch as u64, &mut result, 1000) != 0
        || u32::from_le_bytes([result[8], result[9], result[10], result[11]]) != 1
    {
        return Err(4);
    }
    Ok(())
}

#[cfg(feature = "dw2-scene-policy")]
fn compose_policy_scene(
    scene: Handle,
    scene_pixels: *mut u32,
    a: Option<&Client>,
    b: Option<&Client>,
    c: Option<&Client>,
    held_chip: bool,
) -> Result<(), i32> {
    let scene_slice = unsafe { core::slice::from_raw_parts_mut(scene_pixels, SCENE_W * SCENE_H) };
    let empty = [0u32; 1];
    let (a_copy, b_copy, c_copy) = (a.map(copy), b.map(copy), c.map(copy));
    let a_pixels: &[u32] = a_copy.as_ref().map_or(&empty, |p| p);
    let b_pixels: &[u32] = b_copy.as_ref().map_or(&empty, |p| p);
    let c_pixels: &[u32] = c_copy.as_ref().map_or(&empty, |p| p);
    let mut surfaces = [
        SceneSurface {
            pixels: &empty,
            width: 1,
            height: 1,
            stride: 1,
            x: 0,
            y: 0,
        },
        SceneSurface {
            pixels: &empty,
            width: 1,
            height: 1,
            stride: 1,
            x: 0,
            y: 0,
        },
        SceneSurface {
            pixels: &empty,
            width: 1,
            height: 1,
            stride: 1,
            x: 0,
            y: 0,
        },
    ];
    let mut count = 0usize;
    if let Some(client) = a {
        surfaces[count] = SceneSurface {
            pixels: a_pixels,
            width: client.width,
            height: client.height,
            stride: CLIENT_STRIDE_PIXELS,
            x: A_X,
            y: A_Y,
        };
        count += 1;
    }
    if let Some(client) = b {
        surfaces[count] = SceneSurface {
            pixels: b_pixels,
            width: client.width,
            height: client.height,
            stride: CLIENT_STRIDE_PIXELS,
            x: B_X,
            y: B_Y,
        };
        count += 1;
    }
    if let Some(client) = c {
        surfaces[count] = SceneSurface {
            pixels: c_pixels,
            width: client.width,
            height: client.height,
            stride: CLIENT_STRIDE_PIXELS,
            x: C_X,
            y: C_Y,
        };
        count += 1;
    }

    compose_desktop_scene_xrgb8888(
        scene_slice,
        SCENE_W,
        SCENE_H,
        SCENE_W,
        DesktopRoot {
            background: ROOT_BG,
            spine: SPINE,
            spine_width: 8,
        },
        &surfaces[..count],
    )
    .map_err(|_| 1)?;
    if held_chip {
        for y in 10..16 {
            let row = y * SCENE_W;
            for x in 1..7 {
                scene_slice[row + x] = HOLD_CHIP;
            }
        }
    }
    present_scene(scene)
}

#[cfg(feature = "dw2-scene-policy")]
fn present_scene(scene: Handle) -> Result<(), i32> {
    let damage = encode_damage_rect(0, 0, SCENE_W as u16, SCENE_H as u16);
    if sys_buffer_commit_damage(scene, &damage) != 0 {
        return Err(2);
    }
    let epoch = sys_fb_present(scene, 0, 0);
    if epoch < 1 {
        return Err(3);
    }
    let mut result = [0u8; PRESENT_RESULT_V1_LEN];
    if sys_present_fence_wait(scene, epoch as u64, &mut result, 1000) != 0
        || u32::from_le_bytes([result[8], result[9], result[10], result[11]]) != 1
    {
        return Err(4);
    }
    Ok(())
}

#[cfg(feature = "dw2-scene-policy")]
fn hash_client_pixels(client: &Client) -> u64 {
    let pixels = copy(client);
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for pixel in pixels {
        hash ^= pixel as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(feature = "dw2-scene-policy")]
#[derive(Clone, Copy)]
enum SceneOracle {
    Initial,
    FocusAInitial,
    FocusAHistory,
    FocusB,
    HoldB,
    RestoreB,
    ConfigureB,
    DismissB,
    SpawnC,
    DismissC,
}

#[cfg(feature = "dw2-scene-policy")]
fn expect_scene_pixel(
    scene_pixels: *const u32,
    x: usize,
    y: usize,
    expected: u32,
) -> Result<(), i32> {
    let got = unsafe { *scene_pixels.add(y * SCENE_W + x) };
    if got == expected { Ok(()) } else { Err(1) }
}

#[cfg(feature = "dw2-scene-policy")]
fn dw2_scene_byte_oracle(scene_pixels: *const u32, stage: SceneOracle) -> Result<(), i32> {
    expect_scene_pixel(scene_pixels, 0, 0, SPINE)?;
    expect_scene_pixel(scene_pixels, 12, 12, ROOT_BG)?;
    match stage {
        SceneOracle::Initial => {
            expect_scene_pixel(scene_pixels, A_X, A_Y, A_FILL)?;
            expect_scene_pixel(scene_pixels, B_X, B_Y, B_FILL)
        }
        SceneOracle::FocusAInitial => {
            expect_scene_pixel(scene_pixels, A_X, A_Y, A_EVENT)?;
            expect_scene_pixel(scene_pixels, B_X, B_Y, B_FILL)
        }
        SceneOracle::FocusAHistory | SceneOracle::FocusB => {
            expect_scene_pixel(scene_pixels, A_X, A_Y, A_EVENT)?;
            expect_scene_pixel(scene_pixels, B_X, B_Y, B_EVENT)
        }
        SceneOracle::HoldB => {
            expect_scene_pixel(scene_pixels, 1, 10, HOLD_CHIP)?;
            expect_scene_pixel(scene_pixels, A_X, A_Y, A_EVENT)?;
            expect_scene_pixel(scene_pixels, B_X, B_Y, A_FILL)?;
            expect_scene_pixel(scene_pixels, 60, 52, ROOT_BG)
        }
        SceneOracle::RestoreB => {
            expect_scene_pixel(scene_pixels, 1, 10, SPINE)?;
            expect_scene_pixel(scene_pixels, A_X, A_Y, A_EVENT)?;
            expect_scene_pixel(scene_pixels, B_X, B_Y, B_EVENT)
        }
        SceneOracle::ConfigureB => {
            expect_scene_pixel(scene_pixels, A_X, A_Y, A_EVENT)?;
            expect_scene_pixel(scene_pixels, B_X, B_Y, B_EVENT)?;
            expect_scene_pixel(scene_pixels, B_X + 23, B_Y + 23, B_FILL)?;
            expect_scene_pixel(scene_pixels, B_X + 24, B_Y + 24, ROOT_BG)
        }
        SceneOracle::DismissB => {
            expect_scene_pixel(scene_pixels, A_X, A_Y, A_EVENT)?;
            expect_scene_pixel(scene_pixels, B_X, B_Y, A_FILL)?;
            expect_scene_pixel(scene_pixels, 60, 52, ROOT_BG)
        }
        SceneOracle::SpawnC => {
            expect_scene_pixel(scene_pixels, A_X, A_Y, A_EVENT)?;
            expect_scene_pixel(scene_pixels, B_X, B_Y, A_FILL)?;
            expect_scene_pixel(scene_pixels, C_X, C_Y, C_FILL)
        }
        SceneOracle::DismissC => {
            expect_scene_pixel(scene_pixels, A_X, A_Y, A_EVENT)?;
            expect_scene_pixel(scene_pixels, B_X, B_Y, A_FILL)?;
            expect_scene_pixel(scene_pixels, C_X, C_Y, ROOT_BG)
        }
    }
}

#[cfg(feature = "dw2-scene-policy")]
fn send_configure(
    client: &Client,
    width: u32,
    height: u32,
    serial: u64,
    registry: &mut SurfaceRegistry,
) -> Result<(), i32> {
    if sys_send(client.session, b"C") != 1 {
        return Err(1);
    }
    let request = Request::Configure(ConfigureSurface {
        identity: client.identity,
        width,
        height,
        configure_serial: serial,
    });
    let (bytes, len) = encode_request(7, request);
    if sys_send(client.surface, &bytes[..len]) != len as i64 {
        return Err(2);
    }
    let mut reply_bytes = [0u8; REPLY_LEN];
    if sys_recv(client.surface, &mut reply_bytes) != REPLY_LEN as i64 {
        return Err(3);
    }
    let reply = decode_reply(&reply_bytes).map_err(|_| 4)?;
    if reply.opcode != OP_CONFIGURE
        || reply.request_id != 7
        || reply.status != 0
        || reply.identity != client.identity
    {
        return Err(5);
    }
    if !recv_control(client.session, b'C') {
        return Err(6);
    }
    registry
        .configure_on_channel(
            match request {
                Request::Configure(configure) => configure,
                _ => return Err(7),
            },
            bind_policy_channel(WindowSlot::B, client).map_err(|_| 8)?,
        )
        .map_err(|_| 9)?;
    Ok(())
}

#[cfg(feature = "dw2-scene-policy")]
fn inv_wm1_configure_fuzz(
    registry: &mut SurfaceRegistry,
    a: libneo::atrium_protocol::Identity,
    b: libneo::atrium_protocol::Identity,
) -> Result<(), i32> {
    let peer_named_by_a = libneo::atrium_protocol::Identity {
        surface_id: b.surface_id,
        surface_cap_provenance: a.surface_cap_provenance,
        ..b
    };
    if registry.configure_on_channel(
        ConfigureSurface {
            identity: peer_named_by_a,
            width: 24,
            height: 24,
            configure_serial: 2,
        },
        a.surface_cap_provenance,
    ) != Err(libneo::atrium_protocol::ProtocolError::CapRequired)
    {
        return Err(1);
    }
    let foreign_namespace = libneo::atrium_protocol::Identity {
        namespace_id: NS + 1,
        ..a
    };
    if registry.configure_on_channel(
        ConfigureSurface {
            identity: foreign_namespace,
            width: 24,
            height: 24,
            configure_serial: 3,
        },
        a.surface_cap_provenance,
    ) != Err(libneo::atrium_protocol::ProtocolError::WrongOwner)
    {
        return Err(2);
    }
    if registry.configure_on_channel(
        ConfigureSurface {
            identity: b,
            width: 24,
            height: 24,
            configure_serial: 4,
        },
        a.surface_cap_provenance,
    ) != Err(libneo::atrium_protocol::ProtocolError::CapRequired)
    {
        return Err(3);
    }
    Ok(())
}

fn handle_commit(client: &Client, registry: &mut SurfaceRegistry) -> Result<(), i32> {
    handle_commit_inner(client.surface, registry, client.identity)
}

fn handle_commit_inner(
    surface: Handle,
    registry: &mut SurfaceRegistry,
    identity: libneo::atrium_protocol::Identity,
) -> Result<(), i32> {
    let (header, commit) = match recv_request(surface) {
        Ok((header, Request::Commit(commit))) => (header, commit),
        _ => return Err(1),
    };
    registry
        .commit_on_channel(commit, identity.surface_cap_provenance)
        .map_err(|_| 2)?;
    if !send_reply(surface, OP_COMMIT, header.request_id, 0, identity) {
        return Err(3);
    }
    Ok(())
}

fn handle_destroy(client: &Client, registry: &mut SurfaceRegistry) -> Result<(), i32> {
    let (header, destroy) = match recv_request(client.surface) {
        Ok((header, Request::Destroy(destroy))) => (header, destroy),
        _ => return Err(1),
    };
    registry
        .destroy_on_channel(destroy, client.identity.surface_cap_provenance)
        .map_err(|_| 2)?;
    if registry.is_alive(client.identity)
        || !send_reply(
            client.surface,
            OP_DESTROY_SURFACE,
            header.request_id,
            0,
            client.identity,
        )
    {
        return Err(3);
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
