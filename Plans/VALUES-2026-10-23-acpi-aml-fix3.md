<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# OT-86 Fix 3: every corpus evaluation whose value changes

Branch `claude/acpi-aml-fix1`, draft PR #107. The `acpi+fix1` column of `tools/acpi-aml-grade/report/coverage.json`,
covering each `_STA`/`_INI` evaluation and each device probe, compared before and after. A row is listed when the
rendered outcome differs: a value, or FAIL (any failure class), or "(not evaluated)" (the table that defines it
did not load before). It was generated from the committed reports with a script that diffs the two JSON files.

## The LNot fix alone (`b48a1a8d`, against its parent)

| Set | Evaluation | Before | After |
|---|---|---|---|
| `zenbook-s14-ux5406aa-303` | `\_SB_.PC00.LPCB.EC0_._INI` | FAIL | [Uninitialized] |

No evaluation that completed before changed value.

## The review fix (Chad, relay #48): compare operands convert to the left's type

Against `main` `8ac102b0`: **0 rows**. `coverage.json` is byte-identical before and after, so no evaluation changes
outcome or value: no corpus evaluation compares an Integer or String with a right operand of another type.

The `LAnd`/`LOr` commit (`4779b95d`, against its parent `eed3d4a0`): also **0 rows**, `coverage.json` byte-identical.
It corrects wrong values in principle (`LAnd (One, "0")` was true), but no corpus evaluation that completes reaches
`LAnd`/`LOr` with a String or zero-length Buffer operand.

## All of Fix 3 (this branch against `main` `436674dd`)

97 rows. Every one goes from FAIL or "(not evaluated)" to a result (97 of 97). No evaluation
that completed on `main` changes value, and none that completed now fails.

| Set | Evaluation | Before | After |
|---|---|---|---|
| `hp-elitedesk-800-g6-mini-s21-02.27.00` | `\_SB_.PCI0.LPCB.SIO1._STA` | FAIL | Integer(0) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\HPET._STA` | (not evaluated) | Integer(1) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\VIRD._STA` | (not evaluated) | Integer(0) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.EMM0._INI` | (not evaluated) | [Uninitialized] |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.EMM0._STA` | (not evaluated) | Integer(0) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.FUR0._STA` | (not evaluated) | Integer(0) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.FUR1._STA` | (not evaluated) | Integer(0) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.FUR2._STA` | (not evaluated) | Integer(0) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.FUR3._STA` | (not evaluated) | Integer(0) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.I2CA._STA` | (not evaluated) | Integer(0) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.I2CB._STA` | (not evaluated) | Integer(0) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.I2CC._STA` | (not evaluated) | Integer(0) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.I2CD._STA` | (not evaluated) | Integer(0) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.I2CE._STA` | (not evaluated) | Integer(0) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.I2CF._STA` | (not evaluated) | Integer(0) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.LNKA._STA` | (not evaluated) | Integer(9) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.LNKB._STA` | (not evaluated) | Integer(9) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.LNKC._STA` | (not evaluated) | Integer(9) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.LNKD._STA` | (not evaluated) | Integer(9) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.LNKE._STA` | (not evaluated) | Integer(9) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.LNKF._STA` | (not evaluated) | Integer(9) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.LNKG._STA` | (not evaluated) | Integer(9) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.LNKH._STA` | (not evaluated) | Integer(9) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.PCI0.AMDN._STA` | (not evaluated) | Integer(15) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.PCI0.GP17.APSP._STA` | (not evaluated) | Integer(15) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.PCI0.GP17.VGA_.CAMF._STA` | (not evaluated) | Integer(11) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.PCI0.GP17.VGA_.CAMR._STA` | (not evaluated) | Integer(11) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.PCI0.GP17.VGA_.I2SC._STA` | (not evaluated) | Integer(0) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.PCI0.GP17.VGA_._STA` | (not evaluated) | Integer(15) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.PCI0.GP17.XHC0.RHUB.PRT9._STA` | (not evaluated) | Integer(0) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.PCI0.GP17.XHC0.RHUB.PRTA._STA` | (not evaluated) | Integer(0) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.PCI0.GP17.XHC1._STA` | (not evaluated) | Integer(15) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.PCI0.S900._STA` | (not evaluated) | Integer(15) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.PCI0.SBRG.BRAM._STA` | (not evaluated) | Integer(0) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.PCI0.SBRG.HHMD._STA` | (not evaluated) | Integer(0) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.PCI0.SBRG.PMC1._STA` | (not evaluated) | Integer(0) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.PCI0.SBRG.PMC2._STA` | (not evaluated) | Integer(0) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.PCI0.SBRG.SGPI._STA` | (not evaluated) | Integer(0) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.PCI0.SBRG.SRTC._STA` | (not evaluated) | Integer(0) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.PCI0.SBRG.UAR1._STA` | (not evaluated) | Integer(15) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.PCI0.SBRG.UAR2._STA` | (not evaluated) | Integer(0) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.PCI0.UAR1._STA` | (not evaluated) | Integer(0) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.PCI0.UAR2._STA` | (not evaluated) | Integer(0) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.PCI0.UAR3._STA` | (not evaluated) | Integer(0) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.PCI0.UAR4._STA` | (not evaluated) | Integer(0) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.PCI0._INI` | (not evaluated) | [Uninitialized] |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.PCI0._STA` | (not evaluated) | Integer(15) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.PWRB._STA` | (not evaluated) | Integer(11) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.SPI1._STA` | (not evaluated) | Integer(0) |
| `lenovo-thinkcentre-m75q-gen1-m2fkt21a` | `\_SB_.TPM_._STA` | (not evaluated) | Integer(15) |
| `lenovo-thinkcentre-m910q-m1akt5aa` | `\_SB_.PCI0.LPCB.LPTE._STA` | FAIL | Integer(15) |
| `lenovo-thinkcentre-m910q-m1akt5aa` | `\_SB_.PCI0.LPCB.UAR1._STA` | FAIL | Integer(15) |
| `lenovo-thinkcentre-m910q-m1akt5aa` | `\_SB_.PCI0.LPCB.UAR2._STA` | FAIL | Integer(0) |
| `msi-ms-7e57-1.a70` | `\_SB_.FUR0._STA` | (not evaluated) | Integer(0) |
| `msi-ms-7e57-1.a70` | `\_SB_.FUR1._STA` | (not evaluated) | Integer(0) |
| `msi-ms-7e57-1.a70` | `\_SB_.FUR2._STA` | (not evaluated) | Integer(0) |
| `msi-ms-7e57-1.a70` | `\_SB_.FUR3._STA` | (not evaluated) | Integer(0) |
| `msi-ms-7e57-1.a70` | `\_SB_.FUR4._STA` | (not evaluated) | Integer(0) |
| `msi-ms-7e57-1.a70` | `\_SB_.GPIO._STA` | (not evaluated) | Integer(15) |
| `msi-ms-7e57-1.a70` | `\_SB_.HFP1._STA` | (not evaluated) | Integer(0) |
| `msi-ms-7e57-1.a70` | `\_SB_.HID0._STA` | (not evaluated) | Integer(0) |
| `msi-ms-7e57-1.a70` | `\_SB_.I2CA._STA` | (not evaluated) | Integer(0) |
| `msi-ms-7e57-1.a70` | `\_SB_.I2CB._STA` | (not evaluated) | Integer(0) |
| `msi-ms-7e57-1.a70` | `\_SB_.I2CC._STA` | (not evaluated) | Integer(0) |
| `msi-ms-7e57-1.a70` | `\_SB_.I2CD._STA` | (not evaluated) | Integer(0) |
| `msi-ms-7e57-1.a70` | `\_SB_.I3CA._STA` | (not evaluated) | Integer(0) |
| `msi-ms-7e57-1.a70` | `\_SB_.I3CB._STA` | (not evaluated) | Integer(0) |
| `msi-ms-7e57-1.a70` | `\_SB_.I3CC._STA` | (not evaluated) | Integer(0) |
| `msi-ms-7e57-1.a70` | `\_SB_.I3CD._STA` | (not evaluated) | Integer(0) |
| `msi-ms-7e57-1.a70` | `\_SB_.LNKA._STA` | FAIL | Integer(9) |
| `msi-ms-7e57-1.a70` | `\_SB_.LNKB._STA` | FAIL | Integer(9) |
| `msi-ms-7e57-1.a70` | `\_SB_.LNKC._STA` | FAIL | Integer(9) |
| `msi-ms-7e57-1.a70` | `\_SB_.LNKD._STA` | FAIL | Integer(9) |
| `msi-ms-7e57-1.a70` | `\_SB_.LNKE._STA` | FAIL | Integer(9) |
| `msi-ms-7e57-1.a70` | `\_SB_.LNKF._STA` | FAIL | Integer(9) |
| `msi-ms-7e57-1.a70` | `\_SB_.LNKG._STA` | FAIL | Integer(9) |
| `msi-ms-7e57-1.a70` | `\_SB_.LNKH._STA` | FAIL | Integer(9) |
| `msi-ms-7e57-1.a70` | `\_SB_.PCI0.SBRG.UAR1._STA` | FAIL | Integer(13) |
| `msi-ms-7e57-1.a70` | `\_SB_.PCI0.UAR1._STA` | (not evaluated) | Integer(0) |
| `msi-ms-7e57-1.a70` | `\_SB_.PCI0.UAR2._STA` | (not evaluated) | Integer(0) |
| `msi-ms-7e57-1.a70` | `\_SB_.PCI0.UAR3._STA` | (not evaluated) | Integer(0) |
| `msi-ms-7e57-1.a70` | `\_SB_.PCI0.UAR4._STA` | (not evaluated) | Integer(0) |
| `msi-ms-7e57-1.a70` | `\_SB_.PPKG._STA` | (not evaluated) | Integer(15) |
| `optiplex-7080-1.37.0` | `\_SB_.PEPD._STA` | FAIL | Integer(15) |
| `optiplex-7080-linux-1.37.0` | `\_SB_.PEPD._STA` | FAIL | Integer(15) |
| `optiplex-7080-linux-1.37.0` | `\_TZ_.FN00._STA` | FAIL | Integer(0) |
| `optiplex-7080-linux-1.37.0` | `\_TZ_.FN01._STA` | FAIL | Integer(0) |
| `optiplex-7080-linux-1.37.0` | `\_TZ_.FN02._STA` | FAIL | Integer(0) |
| `optiplex-7080-linux-1.37.0` | `\_TZ_.FN03._STA` | FAIL | Integer(0) |
| `optiplex-7080-linux-1.37.0` | `\_TZ_.FN04._STA` | FAIL | Integer(0) |
| `zenbook-s14-ux5406aa-303` | `\_SB_.PC00.I2C4.TPL0._CRS -> resource_descriptor_list` | FAIL | [I2cSerialBus(I2cSerialBusDescriptor { revision_id: 1, resource_source_index: 0, device_initiated: false, is_consumer: true, is_shared: false, ten_bit_addressing: false, type_revision_id: 1, connection_speed: 400000, slave_address: 16, vendor_data: [], resource_source: "\\_SB.PC00.I2C4" }), Irq(IrqDescriptor { is_consumer: true, trigger: Level, polarity: ActiveLow, is_shared: false, is_wake_capabl… |
| `zenbook-s14-ux5406aa-303` | `\_SB_.PC00.I2C4.TPL0._DSM(hid-i2c,1,0)` | FAIL | Buffer(03) |
| `zenbook-s14-ux5406aa-303` | `\_SB_.PC00.I2C4.TPL0._DSM(hid-i2c,1,1)` | FAIL | Integer(1) |
| `zenbook-s14-ux5406aa-303` | `\_SB_.PC00.I2C5.ETPD._CRS -> resource_descriptor_list` | FAIL | [I2cSerialBus(I2cSerialBusDescriptor { revision_id: 1, resource_source_index: 0, device_initiated: false, is_consumer: true, is_shared: false, ten_bit_addressing: false, type_revision_id: 1, connection_speed: 400000, slave_address: 21, vendor_data: [], resource_source: "\\_SB.PC00.I2C5" }), Irq(IrqDescriptor { is_consumer: true, trigger: Level, polarity: ActiveLow, is_shared: false, is_wake_capabl… |
| `zenbook-s14-ux5406aa-303` | `\_SB_.PC00.I2C5.ETPD._DSM(hid-i2c,1,0)` | FAIL | Buffer(03) |
| `zenbook-s14-ux5406aa-303` | `\_SB_.PC00.I2C5.ETPD._DSM(hid-i2c,1,1)` | FAIL | Integer(1) |
| `zenbook-s14-ux5406aa-303` | `\_SB_.PC00.LPCB.EC0_._INI` | FAIL | [Uninitialized] |
