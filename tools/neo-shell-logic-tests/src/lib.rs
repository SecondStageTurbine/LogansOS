// SPDX-License-Identifier: MPL-2.0
//! Host tests for neo-shell's parser.

#![allow(dead_code)]

#[path = "../../../userland/neo-shell/src/shell_logic.rs"]
mod shell_logic;

#[cfg(test)]
mod tests {
    use super::shell_logic::{
        DirEntryKind, MAX_PATH_BYTES, ObserveStatusV1, ParsedCommand, PathError, SYSSTAT_EFAULT,
        SYSSTAT_EINVAL, SYSSTAT_USER_HALF_LIMIT, SYSSTAT_V1_VERSION, SysStatV1, format_dir_entry,
        parse_command, render_observe_status, render_status, resolve_path, status_error_message,
        valid_spawn_name, validate_sysstat_buffer_shape,
    };

    /// OT-87 R4.4: `install` takes no arguments, and the help lists it.
    #[test]
    fn parses_install() {
        assert_eq!(parse_command("install"), ParsedCommand::Install);
        assert_eq!(parse_command("  install  "), ParsedCommand::Install);
        assert_eq!(
            parse_command("install /dev/sda"),
            ParsedCommand::BadArgs { command: "install" }
        );
        assert!(
            core::str::from_utf8(super::shell_logic::HELP_TEXT)
                .unwrap()
                .contains(" install ")
        );
    }

    #[test]
    fn parses_empty_and_whitespace() {
        assert_eq!(parse_command(""), ParsedCommand::Empty);
        assert_eq!(parse_command("  \t\r\n  "), ParsedCommand::Empty);
    }

    #[test]
    fn parses_exit_without_args() {
        assert_eq!(parse_command("exit"), ParsedCommand::Exit);
        assert_eq!(parse_command("  exit  "), ParsedCommand::Exit);
        assert_eq!(
            parse_command("exit now"),
            ParsedCommand::Unknown { command: "exit" }
        );
    }

    #[test]
    fn parses_arthur_without_args() {
        assert_eq!(parse_command("arthur"), ParsedCommand::Arthur);
        assert_eq!(parse_command("  arthur  "), ParsedCommand::Arthur);
        assert_eq!(
            parse_command("arthur now"),
            ParsedCommand::Unknown { command: "arthur" }
        );
    }

    #[test]
    fn parses_spawn_name_only() {
        assert_eq!(
            parse_command("spawn worker"),
            ParsedCommand::Spawn { name: "worker" }
        );
        assert_eq!(
            parse_command(" spawn neo_worker-1 "),
            ParsedCommand::Spawn {
                name: "neo_worker-1"
            }
        );
        assert_eq!(
            parse_command("spawn worker extra"),
            ParsedCommand::Unknown { command: "spawn" }
        );
    }

    #[test]
    fn rejects_names_outside_the_minimal_module_namespace() {
        assert!(valid_spawn_name("worker"));
        assert!(!valid_spawn_name(""));
        assert!(!valid_spawn_name("../worker"));
        assert!(!valid_spawn_name("worker.shell"));
        assert!(!valid_spawn_name("abcdefghijklmnopqrstuvwxyzabcdefg"));
    }

    #[test]
    fn reports_unknown_commands_by_first_token() {
        assert_eq!(
            parse_command("version now"),
            ParsedCommand::Unknown { command: "version" }
        );
    }

    #[test]
    fn parses_filesystem_builtins() {
        assert_eq!(parse_command("help"), ParsedCommand::Help);
        assert_eq!(parse_command("ls"), ParsedCommand::Ls { path: None });
        assert_eq!(
            parse_command("ls /neofs"),
            ParsedCommand::Ls {
                path: Some("/neofs")
            }
        );
        assert_eq!(
            parse_command("cat hello.txt"),
            ParsedCommand::Cat { path: "hello.txt" }
        );
        assert_eq!(parse_command("pwd"), ParsedCommand::Pwd);
        assert_eq!(
            parse_command("cd ../neofs"),
            ParsedCommand::Cd { path: "../neofs" }
        );
        assert_eq!(
            parse_command("echo hello   arthur"),
            ParsedCommand::Echo {
                text: "hello   arthur"
            }
        );
        assert_eq!(parse_command("echo"), ParsedCommand::Echo { text: "" });
        assert_eq!(parse_command("status"), ParsedCommand::Status);
    }

    #[test]
    fn rejects_bad_builtin_args_without_changing_legacy_unknowns() {
        assert_eq!(
            parse_command("exit now"),
            ParsedCommand::Unknown { command: "exit" }
        );
        assert_eq!(
            parse_command("arthur now"),
            ParsedCommand::Unknown { command: "arthur" }
        );
        assert_eq!(
            parse_command("spawn worker extra"),
            ParsedCommand::Unknown { command: "spawn" }
        );
        assert_eq!(
            parse_command("help me"),
            ParsedCommand::BadArgs { command: "help" }
        );
        assert_eq!(
            parse_command("ls a b"),
            ParsedCommand::BadArgs { command: "ls" }
        );
        assert_eq!(
            parse_command("cat"),
            ParsedCommand::BadArgs { command: "cat" }
        );
        assert_eq!(
            parse_command("cd"),
            ParsedCommand::BadArgs { command: "cd" }
        );
        assert_eq!(
            parse_command("status now"),
            ParsedCommand::BadArgs { command: "status" }
        );
    }

    #[test]
    fn resolves_absolute_and_relative_paths() {
        let cases = [
            ("/", "/", "/"),
            ("/", "neofs", "/neofs"),
            ("/", "neofs/hello.txt", "/neofs/hello.txt"),
            ("/neofs", ".", "/neofs"),
            ("/neofs", "./hello.txt", "/neofs/hello.txt"),
            ("/neofs", "../hello.txt", "/hello.txt"),
            ("/neofs/memory", "..", "/neofs"),
            ("/neofs/memory", ".././../hello.txt", "/hello.txt"),
            ("/", "../../hello.txt", "/hello.txt"),
            ("/neofs/", "sub/", "/neofs/sub"),
            ("/neofs", "/hello.txt", "/hello.txt"),
            ("/neofs", "/neofs/./dir/../hello.txt", "/neofs/hello.txt"),
        ];

        for (cwd, input, expected) in cases {
            let mut out = [0u8; MAX_PATH_BYTES];
            let len = resolve_path(cwd, input, &mut out).unwrap();
            assert_eq!(core::str::from_utf8(&out[..len]).unwrap(), expected);
        }
    }

    #[test]
    fn rejects_invalid_or_oversize_paths() {
        let mut out = [0u8; MAX_PATH_BYTES];
        assert_eq!(resolve_path("/", "", &mut out), Err(PathError::Empty));
        assert_eq!(
            resolve_path("relative", "x", &mut out),
            Err(PathError::Invalid)
        );
        assert_eq!(
            resolve_path("/", "bad\npath", &mut out),
            Err(PathError::Invalid)
        );

        let long_component = "a".repeat(MAX_PATH_BYTES);
        assert_eq!(
            resolve_path("/", &long_component, &mut out),
            Err(PathError::TooLong)
        );
    }

    #[test]
    fn formats_ls_entries_without_allocating() {
        let mut out = [0u8; 32];
        let n = format_dir_entry(b"neofs", DirEntryKind::Directory, &mut out).unwrap();
        assert_eq!(&out[..n], b"neofs/\n");

        let n = format_dir_entry(b"hello.txt", DirEntryKind::Regular, &mut out).unwrap();
        assert_eq!(&out[..n], b"hello.txt\n");

        let mut small = [0u8; 4];
        assert_eq!(
            format_dir_entry(b"hello.txt", DirEntryKind::Regular, &mut small),
            None
        );
    }

    #[test]
    fn renders_status_and_disabled_message_without_allocating() {
        let stats = SysStatV1 {
            version: SYSSTAT_V1_VERSION,
            _pad0: 0,
            process_count: 3,
            frames_used: 1024,
            memory_used_bytes: 4 * 1024 * 1024,
            cores_online: 4,
            _pad1: 0,
            uptime_ticks: 25,
        };
        let mut out = [0u8; 96];
        let n = render_status(stats, &mut out).unwrap();
        assert_eq!(
            &out[..n],
            b"processes: 3   memory: 4096 KiB   cores: 4 online   uptime: 25 ticks\n"
        );

        let mut too_small = [0u8; 16];
        assert_eq!(render_status(stats, &mut too_small), None);
        assert_eq!(
            status_error_message(-38),
            Some(b"observability disabled".as_slice())
        );
    }

    #[test]
    fn renders_observe_status_without_allocating() {
        let stats = ObserveStatusV1 {
            cpu_count: 4,
            context_switches: 9,
            dispatches: 7,
            timer_preemptions: 3,
            voluntary_yields: 2,
            ready_queue_depth: 1,
            ready_queue_high_water: 5,
            run_ticks: 11,
            idle_ticks: 13,
            heap_current_bytes: 8 * 1024,
            heap_high_water_bytes: 16 * 1024,
            heap_alloc_failures: 1,
            frames_used: 100,
            frames_free: 200,
            buddy_largest_free_order: 9,
        };
        let mut out = [0u8; 192];
        let n = render_observe_status(stats, &mut out).unwrap();
        assert_eq!(
            &out[..n],
            b"scheduler: cpus=4 ctx=9 dispatch=7 preempt=3 yield=2 rq=1/5 ticks=11/13\nmemory pressure: heap=8/16 KiB frames=100/200 alloc_fail=1 buddy_order=9\n"
        );

        let mut too_small = [0u8; 32];
        assert_eq!(render_observe_status(stats, &mut too_small), None);
    }

    #[test]
    fn validates_sysstat_buffer_shape() {
        let len = core::mem::size_of::<SysStatV1>();
        assert_eq!(validate_sysstat_buffer_shape(0, len), Err(SYSSTAT_EINVAL));
        assert_eq!(
            validate_sysstat_buffer_shape(0x1000, len - 1),
            Err(SYSSTAT_EINVAL)
        );
        assert_eq!(
            validate_sysstat_buffer_shape(0xFFFF_8000_0000_0000, len),
            Err(SYSSTAT_EFAULT)
        );
        assert_eq!(
            validate_sysstat_buffer_shape(SYSSTAT_USER_HALF_LIMIT - 1, len),
            Err(SYSSTAT_EFAULT)
        );
        assert_eq!(validate_sysstat_buffer_shape(0x1000, len), Ok(()));
    }
}
