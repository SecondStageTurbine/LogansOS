# neo-extract-log

Extracts a NeoPlutarch bare-metal scenario log from a block-device log-sink
region.

Default layout:

- Offset: `0x10000000`
- Region size: recorded in the `NEO_LOG_v1` header
- Header size: 4096 bytes
- Payload: starts immediately after the header

Usage:

```sh
cargo run -p neo-extract-log -- --device /dev/sda --output ahci-scenario.log
```

Use `--offset` only if the scenario was built with a non-default sink offset:

```sh
cargo run -p neo-extract-log -- --device /dev/nvme0n1 --offset 0x10000000 --output nvme.log
```

The tool refuses to extract when the magic header is absent or the build-id
field is zeroed. Run it against the disk that was configured as the log sink,
not the scenario disk.
