// SPDX-License-Identifier: MPL-2.0
//! Host KATs for `kernel/src/drivers/fb/progress_logic.rs`: the progress
//! console's glyph scale, text grid and wrapping cursor; and for
//! `kernel/src/drivers/fb/font.rs`: every character the USB summary prints
//! has a glyph of its own. The real modules are mounted through `#[path]`.

#![allow(dead_code)]

#[path = "../../../kernel/src/drivers/fb/progress_logic.rs"]
mod progress_logic;

#[path = "../../../kernel/src/drivers/fb/font.rs"]
mod font;

#[path = "../../../kernel/src/watchdog_timeline_logic.rs"]
mod watchdog_timeline_logic;

#[cfg(test)]
mod tests {
    use super::progress_logic::{Cursor, RESERVED_BOTTOM, Step, grid, scale_for, text_height};

    /// The Zenbook S14's 2880x1800 panel gets 2x glyphs (16x32): 180 x 56
    /// cells, legible in a phone photo. QEMU's 1280x800 and a 1080p panel
    /// stay 1x.
    #[test]
    fn scale_and_grid_per_panel() {
        assert_eq!(scale_for(1800), 2);
        assert_eq!(grid(2880, 1800, 2), (180, 56));
        assert_eq!(scale_for(800), 1);
        assert_eq!(grid(1280, 800, 1), (160, 50));
        assert_eq!(scale_for(1080), 1);
        assert_eq!(scale_for(1440), 2);
        assert_eq!(scale_for(2160), 3);
        assert_eq!(scale_for(10_000), 4, "capped");
        assert_eq!(scale_for(0), 1);
        assert_eq!(grid(4, 4, 1), (1, 1), "never a zero grid");
        // The phase-indicator band is left free.
        assert_eq!(RESERVED_BOTTOM, 200);
        assert_eq!(grid(2880, text_height(1800), 2), (180, 50));
        assert_eq!(grid(1280, text_height(800), 1), (160, 37));
        assert_eq!(text_height(100), 0);
    }

    #[test]
    fn printable_bytes_draw_left_to_right() {
        let mut c = Cursor::new(4, 3);
        assert_eq!(c.step(b'a'), Step::Draw { row: 0, col: 0 });
        assert_eq!(c.step(b'b'), Step::Draw { row: 0, col: 1 });
        assert_eq!(c.step(b'\r'), Step::Skip);
        assert_eq!(c.step(0x07), Step::Skip, "control bytes draw nothing");
        assert_eq!(c.position(), (0, 2));
    }

    /// A newline moves to the next row and asks for it to be cleared.
    #[test]
    fn newline_clears_the_next_row() {
        let mut c = Cursor::new(4, 3);
        c.step(b'a');
        assert_eq!(c.step(b'\n'), Step::NewLine { clear_row: 1 });
        assert_eq!(c.step(b'x'), Step::Draw { row: 1, col: 0 });
    }

    /// A full line breaks before the next printable byte, which then draws at
    /// column 0 of the next row (callers re-feed it).
    #[test]
    fn long_lines_break_at_the_right_edge() {
        let mut c = Cursor::new(2, 3);
        assert_eq!(c.step(b'a'), Step::Draw { row: 0, col: 0 });
        assert_eq!(c.step(b'b'), Step::Draw { row: 0, col: 1 });
        assert_eq!(c.step(b'c'), Step::NewLine { clear_row: 1 });
        assert_eq!(c.step(b'c'), Step::Draw { row: 1, col: 0 });
    }

    /// The last row wraps to row 0, which is cleared first: the newest line
    /// is always the one above the blank row.
    #[test]
    fn the_last_row_wraps_to_the_top() {
        let mut c = Cursor::new(4, 3);
        assert_eq!(c.step(b'\n'), Step::NewLine { clear_row: 1 });
        assert_eq!(c.step(b'\n'), Step::NewLine { clear_row: 2 });
        assert_eq!(c.step(b'\n'), Step::NewLine { clear_row: 0 });
        assert_eq!(c.step(b'z'), Step::Draw { row: 0, col: 0 });
    }

    /// R1 pass 2: leaf 0xB is trusted only when it exists and reports a
    /// non-zero EBX[15:0]; otherwise leaf 1's initial APIC ID (EBX[31:24]).
    #[test]
    fn panic_cpu_id_validates_leaf_0xb() {
        use super::progress_logic::panic_cpu_id;
        let leaf1 = 0x0300_0800; // initial APIC ID 3
        // Valid leaf 0xB: its x2APIC ID, even above 255.
        assert_eq!(panic_cpu_id(0x16, 0x0002, 0x1_0005, leaf1), 0x1_0005);
        // Leaf present but EBX[15:0] = 0: not supported, fall back.
        assert_eq!(panic_cpu_id(0x16, 0x0000, 0xdead, leaf1), 3);
        assert_eq!(
            panic_cpu_id(0x16, 0xffff_0000, 0xdead, leaf1),
            3,
            "only bits 15:0 count"
        );
        // Leaf beyond the maximum: fall back.
        assert_eq!(panic_cpu_id(0x0a, 0x0002, 0xdead, leaf1), 3);
    }

    /// The Zenbook USB summary is read from a photo of the boot console, so
    /// no character in it may fall back to the `?` glyph. These lines use
    /// every character the summary and the port service print.
    /// The touchpad lane's lines are read from a photo the same way (its
    /// feature also compiles the extra glyphs).
    #[test]
    fn i2c_hid_lines_all_have_glyphs() {
        use super::font::glyph;
        let fallback = glyph('\u{1}');
        let lines = [
            "i2c-hid: no match (oem=BOCHS /BXPC     dev=not-read-or-absent) reason=oem - touchpad not present",
            "i2c-hid: quirk=zenbook-s14-ux5406aa match (oem=_ASUS_/Notebook crs=0x676b3 hiddesc-reg=0x0001 dev=8086:e351)",
            "i2c-hid: pci 00:19.1 8086:e351 pmcsr=D3->D0 cmd=0x0002 bar0=0x4017000000",
            "i2c-hid: lpss reset=0x00000000->0x00000003 dma=in-reset caps=0x00000000 clk=0x80010003",
            "i2c-hid: dw ver=0x3230322a fifo=tx64/rx64 fs-hcnt=78 fs-lcnt=150 sda-hold=0x4 addr=0x15 clk-hz=100000000",
            "i2c-hid: hiddesc read FAILED: abort=0x00000001 (addr-nak)",
            "i2c-hid: hiddesc INVALID: Length(65535) (wrong descriptor register or device)",
            "i2c-hid: rdesc bytes=610 items=300 ids=1m,3d,4o mouse=id1 buttons=2x1/bit0 x=12b/bit8 y=12b/bit20 wheel=yes report-bits=40",
            "i2c-hid: rdesc bytes=610 parse FAILED: Unsupported(164) first=[05, 01, 09, 02]",
            "i2c-hid: power-on=ok reset=sent reset-ack=polled(no-irq) stale=0 irq=gsi105/pin105/low/level vector=0xf6 rirr=Some(0) eoi-suppress=0 mode=polled(irq-never-fired)",
            "i2c-hid: reports=8 id=1 parsed=8 unexpected=0 unknown=0 short=0 empty=1 bogus=0 badlen=0 irq=gsi105 count=9 reads=9 errors=0 dropped=0 delivered=0 mode=irq",
            "i2c-hid: TOUCHPAD CHECK - move a finger on the touchpad and click now (6 s)",
            "i2c-hid: irq masked-storm (16 assertions without data) - polling",
        ];
        for line in lines {
            for c in line.chars() {
                assert!(
                    c == ' ' || !core::ptr::eq(glyph(c), fallback),
                    "{c:?} has no glyph (line {line:?})"
                );
            }
        }
    }

    #[test]
    fn usb_summary_characters_all_have_glyphs() {
        use super::font::glyph;
        let fallback = glyph('\u{1}');
        let lines = [
            "usb-summary: ---- USB input evidence ----",
            "usb-summary: xhci controllers=2 chosen=[1] reason=usb2-device-connected",
            "usb-summary: [0] 00:0d.0 8086:e31f D3->D0 v1.20 ports=4 usb3=1-4 ccs=- xecp-walk-stopped",
            "usb-summary: handoff=forced legctl=0xe0000000 msi=true irqs=12 events=40 cmds=6/6",
            "usb-summary: ports powered=0 reconnected=1/2 running=true",
            "usb-summary: port 3 connected enabled=true speed=low pls=0 raw=0x00200e03",
            "usb-summary: device port=3 slot=1 low 046d:c077 class=00/00/00 driver=hid-mouse",
            "usb-summary: keyboard=none mouse=bound",
            "usb-summary: NO keyboard or mouse after 5 s - holding this screen up to 30 s (plugging one in ends the hold)",
            "usb-hotplug: hub slot=2 port=1 connected status=0x0101 - enumerating, attempt 1",
            "xhci: hub slot=2 marked by configure-endpoint (xhci 1.20) ports=4 code=1",
            "xhci: usblegsup after=0x01000001 verdict=forced usblegctlsts before=0x0000e011 after=0xe0000000",
            "ABCDEFGHIJKLMNOPQRSTUVWXYZ abcdefghijklmnopqrstuvwxyz 0123456789 =_+!#%*;<>:,./-[]()|'\"",
        ];
        for line in lines {
            for c in line.chars() {
                assert!(
                    c == ' ' || !core::ptr::eq(glyph(c), fallback),
                    "{c:?} has no glyph (line {line:?})"
                );
            }
        }
        // `=` is two bars, not the fallback.
        let equals = glyph('=');
        assert_eq!(equals[5], 0b0111_1110);
        assert_eq!(equals[9], 0b0111_1110);
        assert_eq!(equals[7], 0);
    }

    /// The watchdog's startup timeline line: `-` for a milestone not yet
    /// reached, the tick otherwise.
    #[test]
    fn timeline_line_reads_not_yet_then_ticks() {
        use super::watchdog_timeline_logic::{Timeline, mark, write_line};
        let mut empty = String::new();
        write_line(
            &mut empty,
            &Timeline {
                now: 7,
                ..Timeline::default()
            },
        )
        .unwrap();
        assert_eq!(
            empty,
            "start tk now=7 | route#0 first=- last=- | key#0 first=- drop=0 rx=- | arthur=- vocab=-..- model=-"
        );
        let t = Timeline {
            now: 4_210,
            routes: 2,
            route_first: 1_015,
            route_last: 3_990,
            keys: 9,
            key_first: 1_200,
            keys_dropped: 3,
            rx_first: 2_400,
            arthur: 3_100,
            vocab_open: 3_110,
            vocab_closed: 3_120,
            model: 3_150,
        };
        let mut line = String::new();
        write_line(&mut line, &t).unwrap();
        assert_eq!(
            line,
            "start tk now=4210 | route#2 first=1015 last=3990 | key#9 first=1200 drop=3 rx=2400 | arthur=3100 vocab=3110..3120 model=3150"
        );
        // Arthur opened its vocab and has not closed it yet (review item 6).
        let mut reading = String::new();
        write_line(
            &mut reading,
            &Timeline {
                now: 3_115,
                arthur: 3_100,
                vocab_open: 3_110,
                ..Timeline::default()
            },
        )
        .unwrap();
        assert!(
            reading.ends_with("| arthur=3100 vocab=3110..- model=-"),
            "{reading}"
        );
        // Tick 0 is "not yet", so a milestone at tick 0 is stored as 1.
        assert_eq!(mark(0), 1);
        assert_eq!(mark(5), 5);
    }

    /// At the largest values the line still fits the Zenbook's 180 2x
    /// columns and the watchdog's 192-byte line, and every character has a
    /// glyph.
    #[test]
    fn timeline_line_fits_and_has_glyphs() {
        use super::font::glyph;
        use super::watchdog_timeline_logic::{Timeline, write_line};
        let big = 99_999_999u64;
        let t = Timeline {
            now: big,
            routes: 9_999,
            route_first: big,
            route_last: big,
            keys: 999_999,
            key_first: big,
            keys_dropped: 999_999,
            rx_first: big,
            arthur: big,
            vocab_open: big,
            vocab_closed: big,
            model: big,
        };
        let mut line = String::new();
        write_line(&mut line, &t).unwrap();
        assert!(line.len() <= 180, "{} columns: {line}", line.len());
        let fallback = glyph('\u{1}');
        for c in line.chars().chain("-".chars()) {
            assert!(
                c == ' ' || !core::ptr::eq(glyph(c), fallback),
                "{c:?} has no glyph"
            );
        }
    }
}
